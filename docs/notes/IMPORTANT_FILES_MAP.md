# Important Files Map

This is the high-signal map of what each important file actually does.

## Repository-level

| File | Real purpose |
|---|---|
| `README.md` | Top-level project intent, run instructions, and current TUI controls. |
| `backend_rominals/Cargo.toml` | Rust crate definition and runtime dependencies (`reqwest`, `serde`, `ratatui`, `crossterm`). |
| `backend_rominals/Cargo.lock` | Exact dependency resolution for reproducible builds. |

## Backend entry and feature modules

| File | Real purpose |
|---|---|
| `backend_rominals/src/main.rs` | CLI entrypoint; validates optional ticker arg and hands off to TUI runtime. |
| `backend_rominals/src/api/mod.rs` | API module boundary; currently exports Yahoo integration. |
| `backend_rominals/src/api/yahoo.rs` | Yahoo Finance HTTP client + response parsing into `Meta`; includes API-level tests. |
| `backend_rominals/src/tui/mod.rs` | TUI runtime orchestration: terminal setup/cleanup, event loop, quote fetch wiring, and `ratatui` terminal frame rendering via `CrosstermBackend`. |
| `backend_rominals/src/tui/state.rs` | Single source of truth for app UI state (`input`, active ticker, quote, error). |
| `backend_rominals/src/tui/controls.rs` | Key/event handling and ticker normalization rules. |
| `backend_rominals/src/tui/view.rs` | `ratatui` UI composition for header/input/quote/status sections, styled text, and quote table rendering. |

## Notes system (your anti-vibe coding support)

| File | Real purpose |
|---|---|
| `docs/notes/WORK_LOG_AND_RESEARCH.md` | Tracks completed work and all suspected weak/uncertain areas that require research. |
| `docs/notes/IMPORTANT_FILES_MAP.md` | Fast map of important files so you always know where logic lives before editing. |
