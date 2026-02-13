use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

/// ConfigBaseId for Flame Elementium – the primary tracked resource.
pub const FLAME_ELEMENTIUM_ID: &str = "100300";

/// Embedded item database: ConfigBaseId → English name.
/// Generated from TITrack's tlidb_items_seed_en.json.
static ITEMS_JSON: &str = include_str!("items.json");

/// Lazy-initialised item lookup.
fn item_db() -> &'static HashMap<String, String> {
    use std::sync::OnceLock;
    static DB: OnceLock<HashMap<String, String>> = OnceLock::new();
    DB.get_or_init(|| serde_json::from_str(ITEMS_JSON).unwrap_or_default())
}

/// Resolve a ConfigBaseId to the English item name (or "Unknown <id>").
pub fn item_name(config_base_id: &str) -> String {
    item_db()
        .get(config_base_id)
        .cloned()
        .unwrap_or_else(|| format!("Unknown {}", config_base_id))
}

// ── Parsed event types ────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct BagEvent {
    pub page_id: u32,
    pub slot_id: u32,
    pub config_base_id: String,
    pub item_name: String,
    pub num: u32,
    pub is_init: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BagRemoveEvent {
    pub page_id: u32,
    pub slot_id: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextMarker {
    pub proto_name: String,
    pub is_start: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MapEvent {
    pub zone_path: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum LogEvent {
    Bag(BagEvent),
    BagRemove(BagRemoveEvent),
    Context(ContextMarker),
    Map(MapEvent),
}

// ── Inventory state (delta tracking) ──────────────────────────────────

/// A single item stack change detected between log snapshots.
#[derive(Debug, Clone, Serialize)]
pub struct ItemDelta {
    pub config_base_id: String,
    pub item_name: String,
    pub delta: i64,
    pub current: u32,
}

/// Accumulated loot summary for the current session.
#[derive(Debug, Clone, Serialize)]
pub struct LootSummary {
    pub items: Vec<ItemDelta>,
    pub total_events: usize,
}

impl LootSummary {
    /// Return the net Flame Elementium delta from this summary.
    pub fn flame_elementium_delta(&self) -> i64 {
        self.items
            .iter()
            .filter(|i| i.config_base_id == FLAME_ELEMENTIUM_ID)
            .map(|i| i.delta)
            .sum()
    }
}

// ── Inventory pages we care about ─────────────────────────────────────
// PageId 100 = Gear (excluded), 101 = Skill, 102 = Commodity, 103 = Misc
const EXCLUDED_PAGES: &[u32] = &[100];

fn is_tracked_page(page_id: u32) -> bool {
    !EXCLUDED_PAGES.contains(&page_id)
}

// ── Line parsers ──────────────────────────────────────────────────────

fn parse_bag_modify(line: &str) -> Option<BagEvent> {
    // BagMgr@:Modfy BagItem PageId = 102 SlotId = 0 ConfigBaseId = 100300 Num = 671
    if !line.contains("BagMgr@:Modfy") {
        return None;
    }
    let page_id = extract_field(line, "PageId")?;
    if !is_tracked_page(page_id) {
        return None;
    }
    let slot_id = extract_field(line, "SlotId")?;
    let cid = extract_field_str(line, "ConfigBaseId")?;
    let num = extract_field(line, "Num")?;
    Some(BagEvent {
        page_id,
        slot_id,
        item_name: item_name(&cid),
        config_base_id: cid,
        num,
        is_init: false,
    })
}

fn parse_bag_init(line: &str) -> Option<BagEvent> {
    // BagMgr@:InitBagData PageId = 102 SlotId = 0 ConfigBaseId = 100300 Num = 609
    if !line.contains("BagMgr@:InitBagData") {
        return None;
    }
    let page_id = extract_field(line, "PageId")?;
    if !is_tracked_page(page_id) {
        return None;
    }
    let slot_id = extract_field(line, "SlotId")?;
    let cid = extract_field_str(line, "ConfigBaseId")?;
    let num = extract_field(line, "Num")?;
    Some(BagEvent {
        page_id,
        slot_id,
        item_name: item_name(&cid),
        config_base_id: cid,
        num,
        is_init: true,
    })
}

fn parse_bag_remove(line: &str) -> Option<BagRemoveEvent> {
    // BagMgr@:RemoveBagItem PageId = 103 SlotId = 39
    if !line.contains("BagMgr@:RemoveBagItem") {
        return None;
    }
    let page_id = extract_field(line, "PageId")?;
    if !is_tracked_page(page_id) {
        return None;
    }
    let slot_id = extract_field(line, "SlotId")?;
    Some(BagRemoveEvent { page_id, slot_id })
}

fn parse_context_marker(line: &str) -> Option<ContextMarker> {
    // ItemChange@ ProtoName=PickItems start
    if !line.contains("ItemChange@") || !line.contains("ProtoName=") {
        return None;
    }
    let proto_start = line.find("ProtoName=")? + "ProtoName=".len();
    let rest = &line[proto_start..];
    let proto_end = rest.find(' ')?;
    let proto_name = rest[..proto_end].to_string();
    let marker = rest[proto_end..].trim();
    if marker != "start" && marker != "end" {
        return None;
    }
    Some(ContextMarker {
        proto_name,
        is_start: marker == "start",
    })
}

fn parse_map_event(line: &str) -> Option<MapEvent> {
    // SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/...
    if !line.contains("OpenMainWorld END!") {
        return None;
    }
    let prefix = "InMainLevelPath";
    let idx = line.find(prefix)?;
    let rest = &line[idx + prefix.len()..];
    let eq = rest.find('=')?;
    let path = rest[eq + 1..].trim().to_string();
    Some(MapEvent { zone_path: path })
}

// ── Field extraction helpers ──────────────────────────────────────────

fn extract_field(line: &str, name: &str) -> Option<u32> {
    extract_field_str(line, name)?.parse().ok()
}

fn extract_field_str(line: &str, name: &str) -> Option<String> {
    let idx = line.find(name)?;
    let rest = &line[idx + name.len()..];
    // Skip optional whitespace + '=' + whitespace
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    let rest = rest.trim_start();
    // Read until next whitespace or end
    let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
    let val = &rest[..end];
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

// ── Public API ────────────────────────────────────────────────────────

/// Parse a single log line into a typed event (or None).
pub fn parse_line(line: &str) -> Option<LogEvent> {
    if let Some(ev) = parse_bag_modify(line) {
        return Some(LogEvent::Bag(ev));
    }
    if let Some(ev) = parse_bag_init(line) {
        return Some(LogEvent::Bag(ev));
    }
    if let Some(ev) = parse_bag_remove(line) {
        return Some(LogEvent::BagRemove(ev));
    }
    if let Some(ev) = parse_context_marker(line) {
        return Some(LogEvent::Context(ev));
    }
    if let Some(ev) = parse_map_event(line) {
        return Some(LogEvent::Map(ev));
    }
    None
}

/// Parse loot from the most recent PickItems block(s) in the log file.
///
/// Reads the log, finds the last inventory snapshot (InitBagData block from
/// sorting) or picks events, and returns item deltas.
pub fn parse_loot_from_log(log_path: &Path) -> io::Result<LootSummary> {
    let contents = fs::read_to_string(log_path)?;
    let lines: Vec<&str> = contents.lines().collect();

    // Track slot state: (page_id, slot_id) -> (config_base_id, num)
    let mut slot_state: HashMap<(u32, u32), (String, u32)> = HashMap::new();
    // Track net deltas per config_base_id
    let mut deltas: HashMap<String, i64> = HashMap::new();
    let mut total_events: usize = 0;
    let mut in_pickup = false;

    // Find last ResetItemsLayout (sort) to get baseline
    let mut last_reset_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().rev() {
        if line.contains("ItemChange@")
            && line.contains("ProtoName=ResetItemsLayout")
            && line.contains("end")
        {
            last_reset_end = Some(i);
            break;
        }
    }

    // If we found a sort, build baseline from InitBagData lines after it
    let scan_start = last_reset_end.unwrap_or(0);

    for line in &lines[scan_start..] {
        if let Some(ev) = parse_line(line) {
            match ev {
                LogEvent::Bag(ref bag) if bag.is_init => {
                    // Snapshot: set slot state baseline
                    slot_state.insert(
                        (bag.page_id, bag.slot_id),
                        (bag.config_base_id.clone(), bag.num),
                    );
                }
                LogEvent::Context(ref ctx) => {
                    if ctx.proto_name == "PickItems" {
                        in_pickup = ctx.is_start;
                    }
                }
                LogEvent::Bag(ref bag) if !bag.is_init => {
                    let key = (bag.page_id, bag.slot_id);
                    let prev_num = slot_state
                        .get(&key)
                        .filter(|(cid, _)| *cid == bag.config_base_id)
                        .map(|(_, n)| *n as i64)
                        .unwrap_or(0);
                    let delta = bag.num as i64 - prev_num;
                    if in_pickup && delta != 0 {
                        *deltas.entry(bag.config_base_id.clone()).or_insert(0) += delta;
                        total_events += 1;
                    }
                    // Update slot state
                    slot_state.insert(key, (bag.config_base_id.clone(), bag.num));
                }
                LogEvent::BagRemove(ref rem) => {
                    let key = (rem.page_id, rem.slot_id);
                    if let Some((cid, prev_num)) = slot_state.remove(&key) {
                        if in_pickup {
                            *deltas.entry(cid).or_insert(0) -= prev_num as i64;
                            total_events += 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    let mut items: Vec<ItemDelta> = deltas
        .into_iter()
        .filter(|(_, d)| *d != 0)
        .map(|(cid, delta)| {
            let current = slot_state
                .values()
                .filter(|(c, _)| *c == cid)
                .map(|(_, n)| *n)
                .sum();
            ItemDelta {
                item_name: item_name(&cid),
                config_base_id: cid,
                delta,
                current,
            }
        })
        .collect();

    // Sort by absolute delta descending
    items.sort_by(|a, b| b.delta.abs().cmp(&a.delta.abs()));

    Ok(LootSummary {
        items,
        total_events,
    })
}

/// Return the current inventory snapshot from the log file.
///
/// Reads InitBagData entries from the most recent sort and applies any
/// subsequent Modfy / Remove events to produce the current state.
pub fn parse_inventory_from_log(log_path: &Path) -> io::Result<Vec<BagEvent>> {
    let contents = fs::read_to_string(log_path)?;
    let lines: Vec<&str> = contents.lines().collect();

    let mut slot_state: HashMap<(u32, u32), BagEvent> = HashMap::new();

    // Find last sort event
    let mut last_reset_end: Option<usize> = None;
    for (i, line) in lines.iter().enumerate().rev() {
        if line.contains("ItemChange@")
            && line.contains("ProtoName=ResetItemsLayout")
            && line.contains("end")
        {
            last_reset_end = Some(i);
            break;
        }
    }

    let scan_start = last_reset_end.unwrap_or(0);

    for line in &lines[scan_start..] {
        if let Some(ev) = parse_line(line) {
            match ev {
                LogEvent::Bag(bag) => {
                    slot_state.insert((bag.page_id, bag.slot_id), bag);
                }
                LogEvent::BagRemove(rem) => {
                    slot_state.remove(&(rem.page_id, rem.slot_id));
                }
                _ => {}
            }
        }
    }

    let mut items: Vec<BagEvent> = slot_state.into_values().collect();
    items.sort_by(|a, b| {
        a.page_id
            .cmp(&b.page_id)
            .then(a.slot_id.cmp(&b.slot_id))
    });
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_item_name_lookup() {
        assert_eq!(item_name("100300"), "Flame Elementium");
    }

    #[test]
    fn test_item_name_unknown() {
        assert_eq!(item_name("999999999"), "Unknown 999999999");
    }

    #[test]
    fn test_parse_bag_modify() {
        let line = "GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 102 SlotId = 0 ConfigBaseId = 100300 Num = 671";
        let ev = parse_line(line).unwrap();
        match ev {
            LogEvent::Bag(b) => {
                assert_eq!(b.page_id, 102);
                assert_eq!(b.slot_id, 0);
                assert_eq!(b.config_base_id, "100300");
                assert_eq!(b.num, 671);
                assert!(!b.is_init);
                assert_eq!(b.item_name, "Flame Elementium");
            }
            _ => panic!("expected Bag event"),
        }
    }

    #[test]
    fn test_parse_bag_init() {
        let line = "GameLog: Display: [Game] BagMgr@:InitBagData PageId = 102 SlotId = 0 ConfigBaseId = 100300 Num = 609";
        let ev = parse_line(line).unwrap();
        match ev {
            LogEvent::Bag(b) => {
                assert!(b.is_init);
                assert_eq!(b.config_base_id, "100300");
                assert_eq!(b.num, 609);
            }
            _ => panic!("expected Bag event"),
        }
    }

    #[test]
    fn test_parse_bag_remove() {
        let line = "GameLog: Display: [Game] BagMgr@:RemoveBagItem PageId = 103 SlotId = 39";
        let ev = parse_line(line).unwrap();
        match ev {
            LogEvent::BagRemove(r) => {
                assert_eq!(r.page_id, 103);
                assert_eq!(r.slot_id, 39);
            }
            _ => panic!("expected BagRemove event"),
        }
    }

    #[test]
    fn test_parse_context_marker() {
        let line = "GameLog: Display: [Game] ItemChange@ ProtoName=PickItems start";
        let ev = parse_line(line).unwrap();
        match ev {
            LogEvent::Context(c) => {
                assert_eq!(c.proto_name, "PickItems");
                assert!(c.is_start);
            }
            _ => panic!("expected Context event"),
        }
    }

    #[test]
    fn test_parse_map_event() {
        let line = "SceneLevelMgr@ OpenMainWorld END! InMainLevelPath = /Game/Art/Maps/01SD/XZ_YuJinZhiXiBiNanSuo200/test";
        let ev = parse_line(line).unwrap();
        match ev {
            LogEvent::Map(m) => {
                assert!(m.zone_path.contains("XZ_YuJinZhiXiBiNanSuo200"));
            }
            _ => panic!("expected Map event"),
        }
    }

    #[test]
    fn test_excluded_page() {
        let line = "GameLog: Display: [Game] BagMgr@:Modfy BagItem PageId = 100 SlotId = 0 ConfigBaseId = 100300 Num = 1";
        assert!(parse_line(line).is_none());
    }

    #[test]
    fn test_flame_elementium_id_constant() {
        assert_eq!(FLAME_ELEMENTIUM_ID, "100300");
        assert_eq!(item_name(FLAME_ELEMENTIUM_ID), "Flame Elementium");
    }

    #[test]
    fn test_loot_summary_flame_elementium_delta() {
        let summary = LootSummary {
            items: vec![
                ItemDelta {
                    config_base_id: FLAME_ELEMENTIUM_ID.to_string(),
                    item_name: "Flame Elementium".to_string(),
                    delta: 150,
                    current: 500,
                },
                ItemDelta {
                    config_base_id: "200100".to_string(),
                    item_name: "Some Other Item".to_string(),
                    delta: 20,
                    current: 30,
                },
            ],
            total_events: 5,
        };
        assert_eq!(summary.flame_elementium_delta(), 150);
    }

    #[test]
    fn test_loot_summary_flame_elementium_delta_none() {
        let summary = LootSummary {
            items: vec![ItemDelta {
                config_base_id: "200100".to_string(),
                item_name: "Some Other Item".to_string(),
                delta: 20,
                current: 30,
            }],
            total_events: 1,
        };
        assert_eq!(summary.flame_elementium_delta(), 0);
    }
}
