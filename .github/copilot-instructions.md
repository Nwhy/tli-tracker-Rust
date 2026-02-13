# Copilot Instructions for tli-tracker-Rust

## Project Overview

This is a CLI tracker for farming sessions in Torchlight: Infinite, written in Rust. It tracks sessions, drops, and calculates profit per minute. The application provides both a CLI interface and a web UI with overlay support.

## Technology Stack

- **Language**: Rust (Edition 2021)
- **CLI Framework**: clap 4.5 with derive features
- **Web Framework**: axum 0.7 with tokio async runtime
- **Data Serialization**: serde and serde_json
- **Time Handling**: chrono with serde support
- **Storage**: JSON files in user's local data directory

## Project Structure

- `src/main.rs` - CLI command handling and application entry point
- `src/models.rs` - Data models (Session, DropItem)
- `src/storage.rs` - File I/O and data persistence
- `src/web.rs` - Web server and API endpoints
- `scripts/build-appimage.sh` - AppImage build script
- `appimage/` - AppImage configuration and assets

## Build and Test Commands

### Building
```bash
cargo build --release
```

### Testing
```bash
cargo test --verbose
```

### Running Locally
```bash
# Initialize storage
cargo run -- init

# Start a session
cargo run -- start-session --map "Netherrealm" --notes "Test run"

# Add a drop
cargo run -- add-drop --name "Flame Core" --quantity 2 --value 18.5

# End session
cargo run -- end-session

# View summary
cargo run -- summary

# Start web server
cargo run -- serve --host 127.0.0.1 --port 8787
```

### Building AppImage
```bash
chmod +x scripts/build-appimage.sh
./scripts/build-appimage.sh
```

## Code Style and Conventions

### General
- Follow standard Rust conventions and idioms
- Use `rustfmt` for code formatting
- Use `clippy` for linting
- Prefer idiomatic Rust patterns (iterators, pattern matching, etc.)

### Error Handling
- Use `anyhow::Result` for error handling throughout the codebase
- Provide clear, user-friendly error messages

### Data Models
- Models are defined in `src/models.rs`
- All models derive `Debug`, `Clone`, `Serialize`, and `Deserialize`
- Use chrono's `DateTime<Utc>` for all timestamps

### Storage
- Data is stored as JSON in `~/.local/share/tli-tracker/sessions.json`
- The storage module handles all file I/O operations
- Always use the `directories` crate for cross-platform path resolution

### Web API
- Web server runs on configurable host:port (default: 127.0.0.1:8787)
- Static files embedded in binary for the web UI
- API endpoints follow REST conventions
- Use axum's built-in JSON serialization

## Development Workflow

1. Make changes to the appropriate module
2. Run `cargo test --verbose` to ensure tests pass
3. Run `cargo build --release` to verify compilation
4. Test manually using the CLI commands or web interface
5. For AppImage changes, test the build script

## CI/CD

- **CI**: GitHub Actions runs on every push/PR to main
  - Builds the project with `cargo build --verbose`
  - Runs tests with `cargo test --verbose`
  
- **Release**: GitHub Actions automatically builds and releases AppImage on version tags
  - Triggered by pushing tags like `v0.1.0`
  - Can also be triggered manually via workflow_dispatch
  - Creates GitHub releases with the built AppImage

## Important Notes

- The AppImage is the recommended distribution method for end users
- The AppImage automatically starts the web server when launched
- CLI functionality is available when building from source
- All session data is stored in the user's local data directory
- The overlay feature is designed for OBS integration

## When Adding Features

1. **New CLI Commands**: Add to the `Commands` enum in `main.rs` and implement the logic
2. **New API Endpoints**: Add routes in `src/web.rs` following the existing pattern
3. **New Data Fields**: Update models in `src/models.rs` and ensure backward compatibility with existing JSON files
4. **Dependencies**: Only add dependencies when necessary; prefer using existing crates in the ecosystem

## Testing Guidelines

- Write unit tests for business logic in models and storage
- Test CLI commands with various inputs and edge cases
- Ensure data persistence works correctly across sessions
- Test web API endpoints for correct responses and error handling
- Consider backward compatibility when changing data structures
