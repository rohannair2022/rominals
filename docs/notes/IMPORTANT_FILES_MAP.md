# Important Files Map

This is the high-signal map of what each important file actually does.

## Repository-level

| File | Real purpose |
|---|---|
| `README.md` | Top-level project intent, run instructions, and current TUI controls. |
| `backend_rominals/Cargo.toml` | Rust crate definition and runtime dependencies (`reqwest`, `serde`, `serde_json`, `ratatui`, `crossterm`). |
| `backend_rominals/Cargo.lock` | Exact dependency resolution for reproducible builds. |

## Backend entry and feature modules

| File | Real purpose |
|---|---|
| `backend_rominals/src/main.rs` | CLI entrypoint; validates optional ticker arg and hands off to TUI runtime. |
| `backend_rominals/src/api/mod.rs` | API module boundary that exports Yahoo quote and MLX analysis integrations. |
| `backend_rominals/src/api/mlx.rs` | Orchestrates six `mlx-lm` worker sections with a hard one-worker-at-a-time cap (default model: `mlx-community/Qwen3.5-4B-MLX-4bit`), emits per-worker section outputs/status, and normalizes plain-text responses. |
| `backend_rominals/src/api/yahoo.rs` | Yahoo Finance HTTP client + response parsing into `Meta`; includes API-level tests. |
| `backend_rominals/src/tui/mod.rs` | TUI runtime orchestration: terminal lifecycle, event loop, quote fetch wiring, async MLX worker status/section events, and completion handling. |
| `backend_rominals/src/tui/state.rs` | UI state store for ticker input, top-level tabs, per-worker MLX sections (`title/content/scroll`), active worker index, status text, and analysis lifecycle flags. |
| `backend_rominals/src/tui/controls.rs` | Key/event handling, ticker normalization, refresh handling, top-level tab navigation, and MLX worker-section controls (`[`/`]`, `1-6`, per-section scrolling). |
| `backend_rominals/src/tui/view.rs` | `ratatui` composition for Yahoo + MLX tabs, including nested worker tabs inside MLX and independent rendering/scrolling per worker section. |

## Notes system (your anti-vibe coding support)

| File | Real purpose |
|---|---|
| `docs/notes/WORK_LOG_AND_RESEARCH.md` | Tracks completed work and all suspected weak/uncertain areas that require research. |
| `docs/notes/IMPORTANT_FILES_MAP.md` | Fast map of important files so you always know where logic lives before editing. |
