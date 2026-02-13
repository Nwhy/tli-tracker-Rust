mod gui;
mod log_parser;
mod models;
mod storage;

use chrono::Utc;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use models::{DropItem, Session};

#[derive(Parser)]
#[command(name = "tli-tracker", version, about = "Torchlight: Infinite farming tracker")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize local data storage
    Init,
    /// Start a new farming session
    StartSession {
        #[arg(long)]
        map: String,
        #[arg(long)]
        notes: Option<String>,
    },
    /// Add a drop to a session (defaults to active session)
    AddDrop {
        #[arg(long)]
        name: String,
        #[arg(long, default_value_t = 1)]
        quantity: u32,
        #[arg(long)]
        value: f64,
        #[arg(long)]
        session: Option<String>,
    },
    /// End a session (defaults to active session)
    EndSession {
        #[arg(long)]
        session: Option<String>,
    },
    /// List sessions
    List,
    /// Show summary for a session (defaults to active session)
    Summary {
        #[arg(long)]
        session: Option<String>,
    },
    /// Export sessions to a JSON file
    Export {
        #[arg(long)]
        out: String,
    },
    /// Launch standalone GUI application
    Gui,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => {
            let path = storage::ensure_data_file()?;
            println!("Storage initialized at {}", path.display());
        }
        Commands::StartSession { map, notes } => {
            let mut sessions = storage::load_sessions()?;
            let session = Session {
                id: Uuid::new_v4().to_string(),
                map,
                notes,
                start_time: Utc::now(),
                end_time: None,
                drops: Vec::new(),
            };
            sessions.push(session.clone());
            storage::save_sessions(&sessions)?;
            println!("Session started: {}", session.id);
        }
        Commands::AddDrop {
            name,
            quantity,
            value,
            session,
        } => {
            let mut sessions = storage::load_sessions()?;
            let target_id = resolve_session_id(&sessions, session)?;
            let drop = DropItem {
                name,
                quantity,
                value,
            };
            let session = sessions
                .iter_mut()
                .find(|s| s.id == target_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
            session.drops.push(drop);
            let session_id = session.id.clone();
            storage::save_sessions(&sessions)?;
            println!("Drop added to session {}", session_id);
        }
        Commands::EndSession { session } => {
            let mut sessions = storage::load_sessions()?;
            let target_id = resolve_session_id(&sessions, session)?;
            let session = sessions
                .iter_mut()
                .find(|s| s.id == target_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;
            let session_id = session.id.clone();
            if session.end_time.is_some() {
                println!("Session already ended: {}", session_id);
            } else {
                session.end_time = Some(Utc::now());
                storage::save_sessions(&sessions)?;
                println!("Session ended: {}", session_id);
            }
        }
        Commands::List => {
            let sessions = storage::load_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
                return Ok(());
            }
            for session in sessions {
                let status = if session.is_active() { "active" } else { "ended" };
                println!(
                    "{} | {} | {} | drops: {}",
                    session.id,
                    session.map,
                    status,
                    session.drops.len()
                );
            }
        }
        Commands::Summary { session } => {
            let sessions = storage::load_sessions()?;
            let target_id = resolve_session_id(&sessions, session)?;
            let session = sessions
                .iter()
                .find(|s| s.id == target_id)
                .ok_or_else(|| anyhow::anyhow!("Session not found"))?;

            println!("Session: {}", session.id);
            println!("Map: {}", session.map);
            if let Some(notes) = &session.notes {
                println!("Notes: {}", notes);
            }
            println!("Drops: {}", session.drops.len());
            println!("Total value: {:.2}", session.total_value());
            if let Some(minutes) = session.duration_minutes() {
                println!("Duration: {:.2} minutes", minutes);
            }
            if let Some(ppm) = session.profit_per_minute() {
                println!("Profit/min: {:.2}", ppm);
            }
        }
        Commands::Export { out } => {
            let sessions = storage::load_sessions()?;
            storage::export_sessions(&sessions, out)?;
            println!("Exported sessions.");
        }
        Commands::Gui => {
            gui::run()?;
        }
    }

    Ok(())
}

fn resolve_session_id(sessions: &[Session], requested: Option<String>) -> anyhow::Result<String> {
    if let Some(id) = requested {
        return Ok(id);
    }

    let active = sessions.iter().find(|s| s.is_active());
    if let Some(session) = active {
        return Ok(session.id.clone());
    }

    Err(anyhow::anyhow!(
        "No active session found. Specify --session <id>."
    ))
}
