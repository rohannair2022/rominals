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
| `backend_rominals/src/api/mod.rs` | API module boundary that exports Yahoo quote, Alpha Vantage context, and Ollama analysis integrations. |
| `backend_rominals/src/api/alpha_vantage.rs` | Pulls structured fundamentals/financials plus latest `NEWS_SENTIMENT` from Alpha Vantage and builds prompt-ready context JSON with partial-data warnings. |
| `backend_rominals/src/api/ollama.rs` | Orchestrates parallel Ollama section workers and streams a final consolidated thesis to the TUI (with fallback and plain-text cleanup). |
| `backend_rominals/src/api/yahoo.rs` | Yahoo Finance HTTP client + response parsing into `Meta`; includes API-level tests. |
| `backend_rominals/src/tui/mod.rs` | TUI runtime orchestration: terminal lifecycle, event loop, quote fetch wiring, Alpha Vantage bootstrap + dual snapshot summarization events (fundamentals/news), async analysis worker events, and streamed analysis state updates. |
| `backend_rominals/src/tui/state.rs` | Single source of truth for app UI state (`input`, active ticker, selected tab, Yahoo quote, Alpha snapshots, Ollama analysis/loading/error, request ID, per-tab scroll offsets). |
| `backend_rominals/src/tui/controls.rs` | Key/event handling, ticker normalization, refresh handling, and tab navigation (`Tab`, `Shift+Tab`, `←/→`, `F1-F3`) plus per-tab scrolling for Alpha/Ollama. |
| `backend_rominals/src/tui/view.rs` | `ratatui` UI composition for strict 3-tab UX (Yahoo / Alpha Vantage / Ollama), with each source rendered in its own isolated tab panel. |

## Notes system (your anti-vibe coding support)

| File | Real purpose |
|---|---|
| `docs/notes/WORK_LOG_AND_RESEARCH.md` | Tracks completed work and all suspected weak/uncertain areas that require research. |
| `docs/notes/IMPORTANT_FILES_MAP.md` | Fast map of important files so you always know where logic lives before editing. |
