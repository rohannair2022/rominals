# Important Files Map

This is the high-signal map of what each important file actually does.
Current baseline: **MLX-backed analysis pipeline** (Ollama path removed from runtime).

## Repository-level

| File | Real purpose |
|---|---|
| `README.md` | Canonical runbook for the Yahoo + MLX + Finnhub terminal workflow, controls, and runtime environment variables. |
| `backend_rominals/Cargo.toml` | Rust crate definition and runtime dependencies (`reqwest`, `serde`, `serde_json`, `ratatui`, `crossterm`, `ureq`, `chrono`). |
| `backend_rominals/Cargo.lock` | Exact dependency resolution for reproducible builds. |
| `.gitignore` | Excludes local MLX venv artifacts (such as `.venv-mlx`) from source control. |

## Backend entry and feature modules

| File | Real purpose |
|---|---|
| `backend_rominals/src/main.rs` | CLI entrypoint; validates optional ticker arg and hands off to TUI runtime. |
| `backend_rominals/src/api/mod.rs` | API module boundary exposing Yahoo, MLX, and Finnhub service integrations. |
| `backend_rominals/src/api/finnhub.rs` | Finnhub service client + snapshot assembler: fetches supported endpoint datasets per ticker (including `news-sentiment` market sentiment), formats payloads for TUI display, and builds bounded Finnhub context text for MLX prompts. |
| `backend_rominals/src/api/mlx.rs` | Core MLX runtime: starts/reuses a shared `mlx_lm.server`, defines 2 parallel sections (**Macro Outlook** + **Micro Outlook**) with industry-standard framing, builds Yahoo-first prompts, and controls thinking mode via `ROMINALS_MLX_ENABLE_THINKING` (`chat_template_kwargs.enable_thinking`). |
| `backend_rominals/src/api/yahoo.rs` | Yahoo Finance HTTP client + `Meta` parser, plus `build_analysis_context` that converts ticker quote/range/volume data into structured context and soft inference cues for MLX prompts. |
| `backend_rominals/src/tui/mod.rs` | TUI runtime orchestrator: terminal lifecycle, event loop, preload + worker event channel, Finnhub snapshot fetching, and merged Yahoo+Finnhub context handoff into MLX worker execution. |
| `backend_rominals/src/tui/state.rs` | App state model for top-level tabs (`Yahoo`/`MLX`/`Finnhub`), active ticker, MLX/Finnhub section buffers + scroll state, and analysis lifecycle/status fields. |
| `backend_rominals/src/tui/controls.rs` | Keyboard bindings: fetch/refresh, three-tab switching (`F1/F2/F3`), shared section navigation (`[`/`]`, `1-9`) for MLX/Finnhub tabs, and per-tab scroll controls. |
| `backend_rominals/src/tui/view.rs` | `ratatui` renderer for Yahoo quote table, MLX section panels, and Finnhub dataset panels with scrolling, per-endpoint error visibility, plus ASCII table formatting for Market/Insider Sentiment datasets. |
| `backend_rominals/scripts/mlx_stream_test.py` | Minimal stream-debug utility for directly observing live MLX token output in terminal. |

## Notes system (your anti-vibe coding support)

| File | Real purpose |
|---|---|
| `docs/notes/WORK_LOG_AND_RESEARCH.md` | Source-of-truth work ledger, migration notes, and unresolved research risks. |
| `docs/notes/IMPORTANT_FILES_MAP.md` | Fast architecture map so edits land in the right files quickly. |
