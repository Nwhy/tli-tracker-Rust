use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, Utc};
use eframe::egui;

use crate::log_parser::{self, ItemDelta, LogEvent, LootSummary, FLAME_ELEMENTIUM_ID};
use crate::storage;

/// Interval between log re-parses.
const POLL_INTERVAL: Duration = Duration::from_secs(3);

// ── Per-map run tracking ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct MapRun {
    map_name: String,
    start: Instant,
    end: Option<Instant>,
    loot_gained: HashMap<String, i64>,
}

impl MapRun {
    fn duration_secs(&self) -> f64 {
        let end = self.end.unwrap_or_else(Instant::now);
        (end - self.start).as_secs_f64()
    }

    fn total_items(&self) -> i64 {
        self.loot_gained.values().sum()
    }
}

// ── Session ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TrackerSession {
    start: Instant,
    start_wall: DateTime<Utc>,
    cumulative_loot: HashMap<String, i64>,
    runs: Vec<MapRun>,
}

impl TrackerSession {
    fn new() -> Self {
        Self {
            start: Instant::now(),
            start_wall: Utc::now(),
            cumulative_loot: HashMap::new(),
            runs: Vec::new(),
        }
    }

    fn elapsed_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }

    fn total_items(&self) -> i64 {
        self.cumulative_loot.values().sum()
    }

    /// Total Flame Elementium gained during this session.
    fn flame_elementium(&self) -> i64 {
        self.cumulative_loot
            .get(FLAME_ELEMENTIUM_ID)
            .copied()
            .unwrap_or(0)
    }

    /// Flame Elementium gained per hour during this session.
    fn flame_elementium_per_hour(&self) -> f64 {
        let secs = self.elapsed_secs();
        if secs < 1.0 {
            return 0.0;
        }
        self.flame_elementium() as f64 / secs * 3600.0
    }
}

// ── Application state ─────────────────────────────────────────────────

pub struct TrackerApp {
    // Log file
    log_path: Option<PathBuf>,
    log_status: String,

    // Polling
    last_poll: Instant,

    // Current parsed data
    loot: Option<LootSummary>,
    inventory: Vec<log_parser::BagEvent>,
    current_map: Option<String>,

    // Session
    session: Option<TrackerSession>,

    // Previous loot state for delta tracking
    prev_loot: HashMap<String, i64>,

    // UI tab
    active_tab: Tab,

    // File watcher channel
    _watcher: Option<notify::RecommendedWatcher>,
    watch_rx: Option<mpsc::Receiver<()>>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Tab {
    FlameElementium,
    Items,
    Inventory,
    Runs,
}

impl TrackerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let log_path = storage::detect_game_log();
        let log_status = match &log_path {
            Some(p) => format!("Log found: {}", p.display()),
            None => "UE_game.log not found – start Torchlight Infinite with logging enabled"
                .to_string(),
        };

        let mut app = Self {
            log_path,
            log_status,
            last_poll: Instant::now() - POLL_INTERVAL, // trigger immediate first poll
            loot: None,
            inventory: Vec::new(),
            current_map: None,
            session: None,
            prev_loot: HashMap::new(),
            active_tab: Tab::FlameElementium,
            _watcher: None,
            watch_rx: None,
        };

        // Set up file watcher if log exists
        app.setup_watcher();

        // Initial parse
        app.poll_log();

        app
    }

    fn setup_watcher(&mut self) {
        use notify::{RecursiveMode, Watcher};

        if let Some(ref log_path) = self.log_path {
            let (tx, rx) = mpsc::channel();
            let sender = tx;
            let mut watcher =
                notify::recommended_watcher(move |_res: Result<notify::Event, notify::Error>| {
                    let _ = sender.send(());
                })
                .ok();

            if let Some(ref mut w) = watcher {
                let _ = w.watch(log_path, RecursiveMode::NonRecursive);
            }

            self._watcher = watcher;
            self.watch_rx = Some(rx);
        }
    }

    fn poll_log(&mut self) {
        if self.log_path.is_none() {
            // Try to detect again
            self.log_path = storage::detect_game_log();
            if let Some(ref p) = self.log_path {
                self.log_status = format!("Log found: {}", p.display());
                self.setup_watcher();
            }
        }

        if let Some(ref path) = self.log_path {
            let path = path.clone();
            // Parse loot
            match log_parser::parse_loot_from_log(&path) {
                Ok(summary) => {
                    // Track deltas for session
                    if let Some(ref mut session) = self.session {
                        let new_loot: HashMap<String, i64> = summary
                            .items
                            .iter()
                            .map(|i| (i.config_base_id.clone(), i.delta))
                            .collect();

                        // Compute session-relative deltas
                        for (cid, &new_delta) in &new_loot {
                            let prev = self.prev_loot.get(cid).copied().unwrap_or(0);
                            let diff = new_delta - prev;
                            if diff != 0 {
                                *session.cumulative_loot.entry(cid.clone()).or_insert(0) += diff;
                            }
                        }
                        self.prev_loot = new_loot;
                    }

                    self.loot = Some(summary);
                }
                Err(e) => {
                    self.log_status = format!("Error parsing log: {}", e);
                }
            }

            // Parse inventory
            if let Ok(inv) = log_parser::parse_inventory_from_log(&path) {
                self.inventory = inv;
            }

            // Detect current map from log
            self.detect_map(&path);
        }

        self.last_poll = Instant::now();
    }

    fn detect_map(&mut self, path: &std::path::Path) {
        if let Ok(contents) = std::fs::read_to_string(path) {
            // Find last map event
            for line in contents.lines().rev() {
                if let Some(LogEvent::Map(m)) = log_parser::parse_line(line) {
                    // Extract readable map name from path
                    let name = m
                        .zone_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&m.zone_path)
                        .to_string();
                    self.current_map = Some(name);
                    return;
                }
            }
        }
    }

    fn start_session(&mut self) {
        let mut session = TrackerSession::new();

        // Snapshot current loot state
        if let Some(ref loot) = self.loot {
            self.prev_loot = loot
                .items
                .iter()
                .map(|i| (i.config_base_id.clone(), i.delta))
                .collect();
        }
        session.cumulative_loot.clear();

        self.session = Some(session);
    }

    fn stop_session(&mut self) {
        self.session = None;
        self.prev_loot.clear();
    }
}

impl eframe::App for TrackerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Check for file watcher notifications
        let mut should_poll = self.last_poll.elapsed() >= POLL_INTERVAL;
        if let Some(ref rx) = self.watch_rx {
            if rx.try_recv().is_ok() {
                should_poll = true;
            }
        }
        if should_poll {
            self.poll_log();
        }

        // Request repaint periodically for live timer updates
        ctx.request_repaint_after(Duration::from_secs(1));

        // Apply black/white theme
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(egui::Color32::from_gray(230));
        visuals.panel_fill = egui::Color32::from_gray(12);
        visuals.window_fill = egui::Color32::from_gray(18);
        visuals.extreme_bg_color = egui::Color32::from_gray(6);
        visuals.faint_bg_color = egui::Color32::from_gray(22);

        // Widget styling
        visuals.widgets.noninteractive.bg_fill = egui::Color32::from_gray(18);
        visuals.widgets.noninteractive.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_gray(180));
        visuals.widgets.inactive.bg_fill = egui::Color32::from_gray(30);
        visuals.widgets.inactive.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::from_gray(200));
        visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(50);
        visuals.widgets.hovered.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::WHITE);
        visuals.widgets.active.bg_fill = egui::Color32::from_gray(70);
        visuals.widgets.active.fg_stroke =
            egui::Stroke::new(1.0, egui::Color32::WHITE);

        visuals.selection.bg_fill = egui::Color32::from_gray(60);
        visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);

        ctx.set_visuals(visuals);

        // ── Top panel: header ─────────────────────────────────────────
        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("⚡ TLI Tracker")
                        .size(20.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new("Torchlight: Infinite Loot Tracker")
                        .size(12.0)
                        .color(egui::Color32::from_gray(120)),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (icon, color) = if self.log_path.is_some() {
                        ("● LOG OK", egui::Color32::from_gray(200))
                    } else {
                        ("○ NO LOG", egui::Color32::from_gray(100))
                    };
                    ui.label(egui::RichText::new(icon).size(12.0).color(color));
                });
            });
            ui.add_space(4.0);
        });

        // ── Bottom panel: status bar ──────────────────────────────────
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.add_space(2.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&self.log_status)
                        .size(11.0)
                        .color(egui::Color32::from_gray(100)),
                );
            });
            ui.add_space(2.0);
        });

        // ── Central panel ─────────────────────────────────────────────
        egui::CentralPanel::default().show(ctx, |ui| {
            // Session controls + stats
            ui.add_space(8.0);
            self.draw_session_bar(ui);
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            // Tabs
            ui.horizontal(|ui| {
                let tabs = [
                    (Tab::FlameElementium, "Flame Elementium"),
                    (Tab::Items, "Items"),
                    (Tab::Inventory, "Inventory"),
                    (Tab::Runs, "Runs"),
                ];
                for (tab, label) in tabs {
                    let selected = self.active_tab == tab;
                    let text = if selected {
                        egui::RichText::new(label)
                            .size(14.0)
                            .color(egui::Color32::WHITE)
                            .strong()
                    } else {
                        egui::RichText::new(label)
                            .size(14.0)
                            .color(egui::Color32::from_gray(120))
                    };
                    if ui.selectable_label(selected, text).clicked() {
                        self.active_tab = tab;
                    }
                    ui.add_space(4.0);
                }
            });

            ui.add_space(6.0);

            match self.active_tab {
                Tab::FlameElementium => self.draw_fe_tab(ui),
                Tab::Items => self.draw_loot_tab(ui),
                Tab::Inventory => self.draw_inventory_tab(ui),
                Tab::Runs => self.draw_runs_tab(ui),
            }
        });
    }
}

impl TrackerApp {
    fn draw_session_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // Session control
            if self.session.is_some() {
                if ui
                    .button(
                        egui::RichText::new("■ Stop Session")
                            .size(13.0)
                            .color(egui::Color32::WHITE),
                    )
                    .clicked()
                {
                    self.stop_session();
                }
            } else if ui
                .button(
                    egui::RichText::new("▶ Start Session")
                        .size(13.0)
                        .color(egui::Color32::WHITE),
                )
                .clicked()
            {
                self.start_session();
            }

            ui.add_space(16.0);

            // Stats boxes
            let map_display = self
                .current_map
                .as_deref()
                .unwrap_or("-");

            self.draw_stat(ui, "MAP", map_display);

            if let Some(ref session) = self.session {
                let elapsed = session.elapsed_secs();
                let mins = (elapsed / 60.0).floor() as u64;
                let secs = (elapsed % 60.0).floor() as u64;
                let time_str = format!("{:02}:{:02}", mins, secs);
                self.draw_stat(ui, "TIME", &time_str);

                let fe = session.flame_elementium();
                self.draw_stat(ui, "FE", &fe.to_string());

                let fe_per_hour = session.flame_elementium_per_hour();
                self.draw_stat(ui, "FE/HR", &format!("{:.0}", fe_per_hour));

                let total = session.total_items();
                self.draw_stat(ui, "ITEMS", &total.to_string());

                let runs = session.runs.len();
                self.draw_stat(ui, "RUNS", &runs.to_string());
            } else {
                self.draw_stat(ui, "TIME", "--:--");
                self.draw_stat(ui, "FE", "-");
                self.draw_stat(ui, "FE/HR", "-");
                self.draw_stat(ui, "ITEMS", "-");
                self.draw_stat(ui, "RUNS", "-");
            }
        });
    }

    fn draw_stat(&self, ui: &mut egui::Ui, label: &str, value: &str) {
        egui::Frame::new()
            .fill(egui::Color32::from_gray(18))
            .stroke(egui::Stroke::new(1.0, egui::Color32::from_gray(40)))
            .corner_radius(6.0)
            .inner_margin(egui::Margin::symmetric(12, 6))
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        egui::RichText::new(label)
                            .size(10.0)
                            .color(egui::Color32::from_gray(100)),
                    );
                    ui.label(
                        egui::RichText::new(value)
                            .size(16.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                });
            });
        ui.add_space(4.0);
    }

    fn draw_fe_tab(&self, ui: &mut egui::Ui) {
        if let Some(ref session) = self.session {
            let fe = session.flame_elementium();
            let fe_hr = session.flame_elementium_per_hour();
            let elapsed = session.elapsed_secs();
            let mins = (elapsed / 60.0).floor() as u64;
            let secs = (elapsed % 60.0).floor() as u64;

            ui.add_space(8.0);

            // Large FE display
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("🔥 FLAME ELEMENTIUM")
                        .size(18.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("{}", fe))
                        .size(48.0)
                        .color(egui::Color32::WHITE)
                        .strong(),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{:.0} FE / hour", fe_hr))
                        .size(20.0)
                        .color(egui::Color32::from_gray(180)),
                );
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!(
                        "Session time: {:02}:{:02}  •  Total items: {}",
                        mins,
                        secs,
                        session.total_items()
                    ))
                    .size(13.0)
                    .color(egui::Color32::from_gray(120)),
                );
            });
        } else {
            // No session – show FE from log if available
            if let Some(ref loot) = self.loot {
                let fe_delta = loot.flame_elementium_delta();
                ui.add_space(8.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new("🔥 FLAME ELEMENTIUM")
                            .size(18.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new(format!("{}", fe_delta))
                            .size(48.0)
                            .color(egui::Color32::WHITE)
                            .strong(),
                    );
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new("from log (start a session to track FE/hour)")
                            .size(13.0)
                            .color(egui::Color32::from_gray(100)),
                    );
                });
            } else {
                ui.label(
                    egui::RichText::new(
                        "Start a session to track Flame Elementium. Sort inventory in-game to sync baseline.",
                    )
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
                );
            }
        }
    }

    fn draw_loot_tab(&self, ui: &mut egui::Ui) {
        // Show session loot if active, otherwise show log loot
        if let Some(ref session) = self.session {
            if session.cumulative_loot.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "Session active – pick up items in-game. Sort inventory to sync baseline.",
                    )
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
                );
                return;
            }

            ui.label(
                egui::RichText::new("Session Loot")
                    .size(14.0)
                    .color(egui::Color32::from_gray(160))
                    .strong(),
            );
            ui.add_space(4.0);

            let mut items: Vec<_> = session.cumulative_loot.iter().collect();
            items.sort_by(|a, b| b.1.abs().cmp(&a.1.abs()));

            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Grid::new("session_loot_grid")
                        .num_columns(3)
                        .spacing([12.0, 4.0])
                        .striped(true)
                        .show(ui, |ui| {
                            // Header
                            ui.label(
                                egui::RichText::new("Item")
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100))
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("ID")
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100))
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new("Delta")
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100))
                                    .strong(),
                            );
                            ui.end_row();

                            for (cid, &delta) in &items {
                                let name = log_parser::item_name(cid);
                                ui.label(
                                    egui::RichText::new(&name)
                                        .size(13.0)
                                        .color(egui::Color32::WHITE),
                                );
                                ui.label(
                                    egui::RichText::new(*cid)
                                        .size(11.0)
                                        .color(egui::Color32::from_gray(80)),
                                );
                                let (sign, color) = if delta > 0 {
                                    ("+", egui::Color32::from_gray(220))
                                } else {
                                    ("", egui::Color32::from_gray(120))
                                };
                                ui.label(
                                    egui::RichText::new(format!("{}{}", sign, delta))
                                        .size(13.0)
                                        .color(color)
                                        .strong(),
                                );
                                ui.end_row();
                            }
                        });
                });
        } else if let Some(ref loot) = self.loot {
            if loot.items.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No loot detected. Sort your inventory in-game to sync, then pick up items.",
                    )
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
                );
                return;
            }

            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Recent Loot from Log")
                        .size(14.0)
                        .color(egui::Color32::from_gray(160))
                        .strong(),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("{} events", loot.total_events))
                        .size(12.0)
                        .color(egui::Color32::from_gray(80)),
                );
            });
            ui.add_space(4.0);

            self.draw_loot_table(ui, &loot.items);
        } else {
            ui.label(
                egui::RichText::new("Waiting for log data...")
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
            );
        }
    }

    fn draw_loot_table(&self, ui: &mut egui::Ui, items: &[ItemDelta]) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("loot_grid")
                    .num_columns(4)
                    .spacing([12.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        // Header
                        for h in ["Item", "ID", "Delta", "Current"] {
                            ui.label(
                                egui::RichText::new(h)
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100))
                                    .strong(),
                            );
                        }
                        ui.end_row();

                        for item in items {
                            ui.label(
                                egui::RichText::new(&item.item_name)
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(&item.config_base_id)
                                    .size(11.0)
                                    .color(egui::Color32::from_gray(80)),
                            );
                            let (sign, color) = if item.delta > 0 {
                                ("+", egui::Color32::from_gray(220))
                            } else {
                                ("", egui::Color32::from_gray(120))
                            };
                            ui.label(
                                egui::RichText::new(format!("{}{}", sign, item.delta))
                                    .size(13.0)
                                    .color(color)
                                    .strong(),
                            );
                            ui.label(
                                egui::RichText::new(item.current.to_string())
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                            ui.end_row();
                        }
                    });
            });
    }

    fn draw_inventory_tab(&self, ui: &mut egui::Ui) {
        if self.inventory.is_empty() {
            ui.label(
                egui::RichText::new(
                    "No inventory data. Sort your inventory in-game to populate.",
                )
                .size(13.0)
                .color(egui::Color32::from_gray(100)),
            );
            return;
        }

        ui.label(
            egui::RichText::new(format!("Inventory ({} slots)", self.inventory.len()))
                .size(14.0)
                .color(egui::Color32::from_gray(160))
                .strong(),
        );
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                egui::Grid::new("inv_grid")
                    .num_columns(4)
                    .spacing([12.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        for h in ["Item", "Page", "Slot", "Qty"] {
                            ui.label(
                                egui::RichText::new(h)
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(100))
                                    .strong(),
                            );
                        }
                        ui.end_row();

                        for item in &self.inventory {
                            ui.label(
                                egui::RichText::new(&item.item_name)
                                    .size(13.0)
                                    .color(egui::Color32::WHITE),
                            );
                            ui.label(
                                egui::RichText::new(item.page_id.to_string())
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                            ui.label(
                                egui::RichText::new(item.slot_id.to_string())
                                    .size(12.0)
                                    .color(egui::Color32::from_gray(140)),
                            );
                            ui.label(
                                egui::RichText::new(item.num.to_string())
                                    .size(13.0)
                                    .color(egui::Color32::WHITE)
                                    .strong(),
                            );
                            ui.end_row();
                        }
                    });
            });
    }

    fn draw_runs_tab(&self, ui: &mut egui::Ui) {
        if let Some(ref session) = self.session {
            if session.runs.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No map runs recorded this session. Runs are detected from map change events in the log.",
                    )
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!("Map Runs ({})", session.runs.len()))
                        .size(14.0)
                        .color(egui::Color32::from_gray(160))
                        .strong(),
                );
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        egui::Grid::new("runs_grid")
                            .num_columns(3)
                            .spacing([12.0, 4.0])
                            .striped(true)
                            .show(ui, |ui| {
                                for h in ["Map", "Duration", "Items"] {
                                    ui.label(
                                        egui::RichText::new(h)
                                            .size(12.0)
                                            .color(egui::Color32::from_gray(100))
                                            .strong(),
                                    );
                                }
                                ui.end_row();

                                for run in session.runs.iter().rev() {
                                    ui.label(
                                        egui::RichText::new(&run.map_name)
                                            .size(13.0)
                                            .color(egui::Color32::WHITE),
                                    );
                                    let secs = run.duration_secs();
                                    let mins = (secs / 60.0).floor() as u64;
                                    let s = (secs % 60.0).floor() as u64;
                                    ui.label(
                                        egui::RichText::new(format!("{}:{:02}", mins, s))
                                            .size(12.0)
                                            .color(egui::Color32::from_gray(160)),
                                    );
                                    ui.label(
                                        egui::RichText::new(run.total_items().to_string())
                                            .size(13.0)
                                            .color(egui::Color32::WHITE)
                                            .strong(),
                                    );
                                    ui.end_row();
                                }
                            });
                    });
            }

            ui.add_space(12.0);
            ui.separator();
            ui.add_space(8.0);

            // Session summary
            ui.label(
                egui::RichText::new("Session Summary")
                    .size(14.0)
                    .color(egui::Color32::from_gray(160))
                    .strong(),
            );
            ui.add_space(4.0);

            let elapsed = session.elapsed_secs();
            let mins = (elapsed / 60.0).floor() as u64;
            let secs = (elapsed % 60.0).floor() as u64;
            ui.label(
                egui::RichText::new(format!(
                    "Started: {}  |  Duration: {}:{:02}  |  FE: {}  |  FE/hr: {:.0}  |  Total items: {}",
                    session.start_wall.with_timezone(&Local).format("%H:%M:%S"),
                    mins,
                    secs,
                    session.flame_elementium(),
                    session.flame_elementium_per_hour(),
                    session.total_items()
                ))
                .size(12.0)
                .color(egui::Color32::from_gray(140)),
            );
        } else {
            ui.label(
                egui::RichText::new("Start a session to track map runs.")
                    .size(13.0)
                    .color(egui::Color32::from_gray(100)),
            );
        }
    }
}

/// Launch the standalone GUI application.
pub fn run() -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([640.0, 400.0])
            .with_title("TLI Tracker – Torchlight: Infinite"),
        ..Default::default()
    };

    eframe::run_native(
        "TLI Tracker",
        options,
        Box::new(|cc| Ok(Box::new(TrackerApp::new(cc)))),
    )
    .map_err(|e| anyhow::anyhow!("GUI error: {}", e))?;

    Ok(())
}
