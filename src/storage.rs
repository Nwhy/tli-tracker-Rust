use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde_json::json;

use crate::models::Session;

pub fn data_file_path() -> io::Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("com", "tli", "tli-tracker")
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "Failed to resolve data directory"))?;
    let data_dir = proj_dirs.data_local_dir();
    fs::create_dir_all(data_dir)?;
    Ok(data_dir.join("sessions.json"))
}

pub fn ensure_data_file() -> io::Result<PathBuf> {
    let path = data_file_path()?;
    if !path.exists() {
        let mut file = fs::File::create(&path)?;
        let initial = json!({ "sessions": [] });
        file.write_all(initial.to_string().as_bytes())?;
    }
    Ok(path)
}

pub fn load_sessions() -> io::Result<Vec<Session>> {
    let path = ensure_data_file()?;
    let mut file = fs::File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let value: serde_json::Value = serde_json::from_str(&contents)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let sessions_value = value.get("sessions").cloned().unwrap_or_else(|| json!([]));
    let sessions: Vec<Session> = serde_json::from_value(sessions_value)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    Ok(sessions)
}

pub fn save_sessions(sessions: &[Session]) -> io::Result<()> {
    let path = ensure_data_file()?;
    let wrapper = json!({ "sessions": sessions });
    let pretty = serde_json::to_string_pretty(&wrapper)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, pretty)?;
    Ok(())
}

pub fn export_sessions<P: AsRef<Path>>(sessions: &[Session], path: P) -> io::Result<()> {
    let pretty = serde_json::to_string_pretty(&sessions)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    fs::write(path, pretty)?;
    Ok(())
}

/// Torchlight Infinite Steam AppID
const TLI_APP_ID: &str = "1974050";

/// Relative path from a Steam library root to the UE_game.log file.
const TLI_LOG_RELATIVE: &str =
    "steamapps/common/Torchlight Infinite/UE_game/TorchLight/Saved/Logs/UE_game.log";

/// Detect the `UE_game.log` file produced by Torchlight Infinite.
///
/// The game writes this log to its installation directory under
/// `<steam-library>/steamapps/common/Torchlight Infinite/UE_game/TorchLight/Saved/Logs/UE_game.log`.
///
/// On Linux with Steam Proton the installation is still inside `steamapps/common/`
/// (the Proton compatdata prefix holds only the virtual Windows user-profile,
/// not the game binaries).
///
/// Several well-known Steam library root locations are probed, and any
/// additional libraries listed in `libraryfolders.vdf` are also searched.
pub fn detect_game_log() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let home = Path::new(&home);

    let steam_roots: Vec<PathBuf> = vec![
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];

    // Check default Steam roots first
    for root in &steam_roots {
        let candidate = root.join(TLI_LOG_RELATIVE);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    // Search additional library folders referenced in libraryfolders.vdf
    for root in &steam_roots {
        let library_file = root.join("steamapps/libraryfolders.vdf");
        if let Some(paths) = parse_library_folders(&library_file) {
            for lib_path in paths {
                let candidate = lib_path.join(TLI_LOG_RELATIVE);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Detect the Torchlight Infinite game data directory (Proton prefix).
///
/// On Linux with Steam Proton the user data lives under
/// `<steam-library>/steamapps/compatdata/1974050/pfx/drive_c/users/steamuser/AppData/LocalLow/XD Entertainment/TorchLight Infinite`.
pub fn detect_game_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let home = Path::new(&home);

    let steam_roots: Vec<PathBuf> = vec![
        home.join(".steam/steam"),
        home.join(".local/share/Steam"),
        home.join(".var/app/com.valvesoftware.Steam/data/Steam"),
    ];

    for root in &steam_roots {
        let candidate = root
            .join("steamapps/compatdata")
            .join(TLI_APP_ID)
            .join("pfx/drive_c/users/steamuser/AppData/LocalLow/XD Entertainment/TorchLight Infinite");
        if candidate.is_dir() {
            return Some(candidate);
        }
    }

    // Also search libraryfolders.vdf-referenced libraries
    for root in &steam_roots {
        let library_file = root.join("steamapps/libraryfolders.vdf");
        if let Some(paths) = parse_library_folders(&library_file) {
            for lib_path in paths {
                let candidate = lib_path
                    .join("steamapps/compatdata")
                    .join(TLI_APP_ID)
                    .join("pfx/drive_c/users/steamuser/AppData/LocalLow/XD Entertainment/TorchLight Infinite");
                if candidate.is_dir() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

/// Minimal parser for Steam's `libraryfolders.vdf` to extract library paths.
fn parse_library_folders(vdf_path: &Path) -> Option<Vec<PathBuf>> {
    let contents = fs::read_to_string(vdf_path).ok()?;
    let mut paths = Vec::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("\"path\"") {
            if let Some(val) = trimmed.split('"').nth(3) {
                paths.push(PathBuf::from(val));
            }
        }
    }
    if paths.is_empty() { None } else { Some(paths) }
}
