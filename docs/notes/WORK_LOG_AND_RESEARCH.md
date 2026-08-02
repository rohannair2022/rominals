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
| 2026-08-01 | Added Finnhub `news-sentiment` integration (`Market Sentiment`) and converted both Market Sentiment + Insider Sentiment datasets into compact ASCII tables in the Finnhub terminal tab for faster scanning. Also included the new sentiment dataset in MLX supplemental context. | `backend_rominals/src/api/finnhub.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | `news-sentiment` can return entitlement errors on free-tier keys; behavior depends on Finnhub plan access. |
| 2026-08-01 | Added a dedicated **Finnhub** TUI interface (third tab) with per-endpoint section browsing/scrolling, and wired full Finnhub snapshot data into MLX prompt context alongside Yahoo snapshot context for each ticker run. | `backend_rominals/src/api/finnhub.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `backend_rominals/Cargo.toml`, `README.md`, `docs/notes/IMPORTANT_FILES_MAP.md` | High | Some Finnhub endpoints are paid-tier and can return 403s; UI now surfaces those per endpoint and continues with available datasets. |
| 2026-08-01 | Added a new Finnhub API service module with endpoint wrappers for stock profile, news, company news, peers, insider activity, financials/filings, earnings signals, USPTO patents, lobbying, and USA spending. Auth now comes from `ROMINALS_FINNHUB_API_KEY` or `FINNHUB_API_KEY` (not hardcoded). | `backend_rominals/src/api/mod.rs`, `backend_rominals/src/api/finnhub.rs`, `README.md`, `docs/notes/IMPORTANT_FILES_MAP.md` | High | Some endpoint paths vary by Finnhub plan/version; if an endpoint returns 404/403, path or plan entitlement may need adjustment. |
| 2026-08-01 | Refactored MLX analysis pipeline from 6 workers to 2 industry-standard sections run in parallel by default: **Macro Outlook** and **Micro Outlook**. Updated prompts/objectives, worker limits, section hotkeys, and UI/docs labels. | `backend_rominals/src/api/mlx.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `README.md`, `docs/notes/IMPORTANT_FILES_MAP.md` | High | Current architecture supports two concurrent sections; scaling beyond 2 will need throughput/cost tuning per model size. |
| 2026-08-01 | Added runtime switch to disable reasoning output by default (`ROMINALS_MLX_ENABLE_THINKING=false`) and forward `chat_template_kwargs.enable_thinking` to `mlx_lm.server` per request. | `backend_rominals/src/api/mlx.rs`, `README.md` | High | Some MLX/model builds may ignore request-level chat template kwargs; behavior depends on backend compatibility. |
| 2026-08-01 | Rewrote MLX worker prompts to be Yahoo-context-first: each analysis run now injects a structured ticker snapshot (price action, 52-week positioning, liquidity regime, and soft inference cues) before generation, plus stricter prompt rules for missing valuation fields (P/S, EV, EV/EBITDA). | `backend_rominals/src/api/yahoo.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/api/mlx.rs` | High | Sector-relative baselines are still missing, so valuation interpretation remains heuristic until a fundamentals source is added. |
| 2026-07-25 | Migrated analysis engine from Ollama to MLX: removed Ollama path, introduced `mlx_lm.server`-backed worker pipeline, switched analysis tab/state/UI copy from Ollama to MLX, and moved runtime config to `ROMINALS_MLX_*` env vars. | `backend_rominals/src/api/mod.rs`, `backend_rominals/src/api/mlx.rs`, `backend_rominals/src/api/ollama.rs`, `backend_rominals/src/tui/mod.rs`, `backend_rominals/src/tui/state.rs`, `backend_rominals/src/tui/controls.rs`, `backend_rominals/src/tui/view.rs`, `README.md` | High | Initial implementation launched with conservative single-worker cap; later expanded to two-section parallel flow. |
| 2026-07-25 | Added isolated MLX runtime via `.venv-mlx` so `mlx-lm` can run without polluting global Python packages; documented `ROMINALS_MLX_PYTHON_BIN` usage for explicit interpreter control. | `.gitignore`, `README.md` | High | The shell still needs `ROMINALS_MLX_PYTHON_BIN` exported unless a shell profile or launch script sets it automatically. |
| 2026-08-01 | Rewrote Obsidian notes to make MLX the source-of-truth architecture and remove stale Ollama-first framing from project notes. | `docs/notes/WORK_LOG_AND_RESEARCH.md`, `docs/notes/IMPORTANT_FILES_MAP.md` | High | None |

## Research queue (possible vibe-coded risk)

| Added on | Topic to research | Why this is a risk | Priority | Status |
|---|---|---|---|---|
| 2026-08-01 | Finnhub `news-sentiment` entitlement handling | Some keys/plans return "You don't have access to this resource." for market sentiment; current UI surfaces the error but does not provide a plan-aware fallback metric. | Medium | Open |
| 2026-08-01 | Sector-relative valuation grounding | The prompt now references valuation interpretations (including P/S + EV patterns), but Yahoo chart meta alone does not provide sector medians or full comp fundamentals for robust relative calls. | High | Open |
| 2026-08-01 | Multi-worker scaling beyond 2 sections | Runtime now supports two concurrent sections by default; expanding past 2 may require adaptive concurrency and model-size-based scheduling. | Medium | Open |
| 2026-08-01 | MLX server lifecycle resilience | `mlx_lm.server` is long-lived and shut down on graceful exit; abrupt termination can still leave orphan process edge cases depending on runtime failure mode. | Medium | Open |
| 2026-08-01 | Optional reasoning stream policy in TUI | Current stream preserves reasoning and content boundaries (`<think>` sections); some users may want a final-answer-only view to reduce UI noise. | Low | Open |
| 2026-07-12 | Non-blocking quote fetch in TUI loop | `reqwest::blocking` runs in input loop, so Yahoo network latency can still freeze interaction. | High | Open |

## Research outcomes

| Date | Topic | Decision | Follow-up change |
|---|---|---|---|
| 2026-07-25 | Ollama vs MLX runtime direction | Standardized on local MLX execution (`mlx_lm.server`) as the primary analysis backend and removed Ollama integration from runtime surfaces. | Completed across `backend_rominals/src/api/mlx.rs`, TUI modules, and `README.md`. |
