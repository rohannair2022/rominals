# Anti-Vibe Coding Skill — Work Log & Research Queue

Use this file every time you code.

## Rule for each change

Before marking work as done, write:
- **What changed**
- **Why was it needed**
- **How sure you are** (High / Medium / Low)
- **What still feels uncertain**

If confidence is not **High**, add an item to the research queue.

## Work done log

| Date | Change summary | Files touched | Confidence | Uncertainty |
|---|---|---|---|---|
| 2026-07-25 | Added isolated MLX runtime setup via `.venv-mlx`, validated generation from that venv, and documented + ignored the venv path so the app can use `ROMINALS_MLX_PYTHON_BIN` without polluting global Python packages. | `.gitignore`, `README.md` | High | `ROMINALS_MLX_PYTHON_BIN` must be exported in each new shell session unless persisted in shell profile. |
| 2026-07-25 | Replaced Ollama infrastructure with local `mlx-lm` worker execution using default model `mlx-community/Qwen3.5-4B-MLX-4bit`, renamed TUI analysis surfaces from Ollama to MLX, and updated runtime/docs to new MLX environment variables. Also validated by running a live `mlx_lm generate` call and backend test suite. | `backend_rominals/src/api/mod.rs`, `backend_rominals/src/api/mlx.rs`, `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `README.md`, `docs/notes/IMPORTANT_FILES_MAP.md` | High | Python package conflicts may exist in the shared global environment; use a virtual environment if this machine runs other Python toolchains. |
| 2026-07-19 | Replaced final Ollama synthesis view with worker-first output: each of the 6 parallel workers now renders its own section in the terminal, with dedicated worker navigation and per-section scroll state. | `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | Worker section output arrives on completion, not token-by-token stream per worker. |
| 2026-07-19 | Removed Alpha Vantage integration completely from the terminal stack: deleted endpoint module, removed Alpha fetch/event/state paths, and collapsed UI from 3 tabs to Yahoo + Ollama only. | `backend_rominals/src/api/mod.rs`, `backend_rominals/src/api/alpha_vantage.rs`, `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | None |
| 2026-07-19 | Reworked the UI into strict source-separated tabs exactly as requested: Yahoo tab, Alpha Vantage tab, and Ollama tab. Added keyboard tab switching and independent scroll state for Alpha vs Ollama content. | `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `backend_rominals/src/tui/mod.rs`, `README.md` | High | Alpha tab currently combines fundamentals + news in one scroll body; if needed this can split into sub-tabs later. |
| 2026-07-19 | Added a second independent Alpha Vantage terminal snapshot and split context into two separate panels (Fundamentals + News) that render independently of Ollama analysis output. | `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | Snapshot panels are currently fixed-height and non-scrollable; heavy news headline sets can truncate. |
| 2026-07-19 | Made Alpha Vantage context visible in-terminal by adding a persistent snapshot block (company, valuation, cash/debt, latest sentiment headline, warnings) at the top of the analysis pane, separate from Ollama synthesis stream. | `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | Snapshot currently surfaces the latest news item plus warning count; deeper multi-headline rendering may still be useful. |
| 2026-07-19 | Extended Alpha Vantage ingestion to include NEWS_SENTIMENT and made snapshot collection resilient: endpoint failures/rate-limit responses now degrade to partial context with warnings instead of aborting the entire analysis context. | `backend_rominals/src/api/alpha_vantage.rs`, `README.md` | High | Free-tier throughput is still constrained; multi-ticker burst usage may still produce warning-heavy contexts. |
| 2026-07-19 | Started "solid terminal" upgrade: added Alpha Vantage snapshot ingestion, wired analysis bootstrap status into the TUI, replaced single-shot analysis with parallel Ollama section workers plus streamed final synthesis, and tuned event-loop responsiveness for smoother live updates. | `backend_rominals/src/api/alpha_vantage.rs`, `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/api/mod.rs`, `backend_rominals/src/tui/mod.rs`, `README.md` | Medium | Alpha Vantage free-tier limits (5 calls/min) can throttle rapid ticker switching; may need caching/throttling layer next. |
| 2026-07-18 | Added Ollama company analysis on each ticker fetch, then hardened the TUI flow with async analysis execution, token streaming, scrollable analysis pane, and output cleanup for markdown artifacts. | `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/api/mod.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `backend_rominals/Cargo.toml`, `README.md` | Medium | Ollama web-search responses can still vary in latency/quality by model-side tool behavior. |
| 2026-07-12 | Split TUI into focused modules (controls/state/view/runtime) and kept CLI arg parsing in `main.rs`. | `backend_rominals/src/main.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs` | High | None |
| 2026-07-13 | Migrated terminal rendering to `ratatui` with structured panels (header/input/quote/status), colorized market change/error states, and table-based quote display. Kept `crossterm` for raw mode, screen lifecycle, and input events via `CrosstermBackend`. | `backend_rominals/Cargo.toml`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/view.rs`, `backend_rominals/src/tui/controls.rs` | High | None |

## Research queue (possible vibe-coded risk)

| Added on | Topic to research | Why this is a risk | Priority | Status |
|---|---|---|---|---|
| 2026-07-12 | Non-blocking quote fetch in TUI loop | `reqwest::blocking` runs in input loop, so network latency can freeze interaction. | High | Open |
| 2026-07-18 | Streaming markdown-to-TUI rendering policy | Current cleanup strips markdown markers for readability; a richer renderer may preserve semantic emphasis better. | Low | Open |
| 2026-07-12 | Better TUI renderer abstraction | Full-screen clear every frame is simple but may cause flicker and poor extensibility as UI grows. | Medium | Closed (Migrated to `ratatui` on 2026-07-13) |
| 2026-07-12 | Error model cleanup | `Box<dyn Error>` is convenient but weak for explicit app-level error handling paths. | Medium | Open |

## Research outcomes

| Date | Topic | Decision | Follow-up change |
|---|---|---|---|
| 2026-07-13 | Better TUI renderer abstraction | Adopted `ratatui` for layout/widgets/styling while keeping `crossterm` backend for terminal and events. | Completed in `backend_rominals/src/tui/mod.rs` and `backend_rominals/src/tui/view.rs` |
