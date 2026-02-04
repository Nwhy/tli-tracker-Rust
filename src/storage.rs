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
