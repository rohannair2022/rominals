# Rominals Architecture (End-to-End)

This project is a **Rust terminal application** (`backend_rominals`) that combines:

- live Yahoo market data,
- multi-endpoint Finnhub datasets,
- local MLX worker analysis.

It also includes **Python helper scripts** in `backend_rominals/scripts/` for MLX streaming and URL-read experiments.

## Repository layout

```text
rominals/
├── backend_rominals/
│   ├── src/
│   │   ├── main.rs                # CLI arg parsing + TUI entrypoint
│   │   ├── tui/
│   │   │   ├── mod.rs             # app loop, event handling, threading, orchestration
│   │   │   ├── state.rs           # central App state model
│   │   │   ├── controls.rs        # keyboard controls and ticker normalization
│   │   │   └── view.rs            # ratatui rendering (Yahoo/Finnhub)
│   │   └── api/
│   │       ├── yahoo.rs           # Yahoo fetch + candle parsing + analysis context
│   │       ├── finnhub.rs         # Finnhub client + 12 datasets + scoped context
│   │       └── mlx.rs             # MLX server lifecycle + parallel worker generation
│   └── scripts/
│       ├── mlx_stream_test.py     # direct token streaming test
│       └── mlx_link_read_test.py  # URL reading behavior check
└── docs/
    └── notes/                     # research/work logs
```

## Runtime architecture (component view)

```mermaid
flowchart LR
    U[User Keyboard Input] --> M[main.rs]
    M --> T[run_tui]

    T --> C[controls.rs handle_event]
    T --> V[view.rs draw_ui]
    T --> S[state.rs App]

    C -->|Enter / Ctrl+R| F[fetch_and_store_quote]
    T -->|every ~2s if active ticker| F

    F -->|quick refresh| Y1[yahoo::fetch_quote_snapshot]
    F -->|full analysis request| QA[queue_analysis_request thread]

    QA --> YT[Yahoo fetch sub-thread]
    QA --> FF[finnhub::fetch_finnhub_snapshot]
    FF --> FE1[12 Finnhub endpoints]
    FF --> FE2[optional link scraping + truncation]

    QA --> AC[mlx::analyze_company_workers]
    AC --> ES[ensure_server_started]
    ES --> MS[Python subprocess: mlx_lm.server]
    AC --> WP[run_parallel_workers: N threads]
    WP -->|HTTP streaming| MS

    WP --> CH[Worker events channel]
    QA --> AE[AnalysisEvent channel to main thread]
    CH --> AE
    AE --> T

    T -->|drain_analysis_events| S
    S --> V
```

## Threading and parallelism map

| Execution unit                     | Type                                       | Spawned from             | Runs what                                                                               | Parallel behavior                    |
| ---------------------------------- | ------------------------------------------ | ------------------------ | --------------------------------------------------------------------------------------- | ------------------------------------ |
| TUI loop                           | Main OS thread                             | Process start            | Input polling, drawing, periodic Yahoo stream refresh, event draining                   | Single-threaded loop                 |
| MLX preload thread (optional)      | Rust thread                                | `queue_model_preload`    | Starts/warms local `mlx_lm.server` when app starts with no initial ticker               | Runs in background once              |
| Analysis request thread            | Rust thread per explicit fetch             | `queue_analysis_request` | Orchestrates Yahoo+Finnhub fetch, then MLX analysis workers                             | One per explicit analysis trigger    |
| Yahoo sub-thread (inside analysis) | Rust thread                                | Analysis thread          | Fetches Yahoo snapshot concurrently                                                     | Runs in parallel with Finnhub fetch  |
| Finnhub fetch path                 | Analysis thread itself                     | Analysis thread          | Calls 12 Finnhub endpoints, builds macro/micro context, optional URL snippet extraction | Sequential in current implementation |
| MLX worker pool                    | 1..N Rust threads (`N` clamped, default 2) | `run_parallel_workers`   | Generates section outputs (Macro/Micro), streams chunks                                 | Workers run concurrently             |
| MLX server                         | Python subprocess                          | `ensure_server_started`  | Hosts local HTTP inference server                                                       | Shared by all workers; loaded once   |

## Per-request execution timeline (explicit analysis fetch)

```mermaid
sequenceDiagram
    participant UI as Main Thread (TUI loop)
    participant AT as Analysis Thread
    participant YT as Yahoo Sub-thread
    participant FH as Finnhub API path
    participant MW as MLX Worker Pool
    participant MS as mlx_lm.server (Python)

    UI->>AT: spawn analysis request (request_id)
    AT-->>UI: Status("Fetching Yahoo snapshot + Finnhub datasets...")

    par Yahoo + Finnhub in parallel
        AT->>YT: spawn Yahoo fetch
        YT->>YT: fetch_quote_snapshot(range, interval)
        YT-->>AT: YahooComplete(result)
    and
        AT->>FH: fetch_finnhub_snapshot(symbol)
        FH->>FH: 12 endpoint calls (serial)
        FH->>FH: build macro/micro context
        FH->>FH: optional URL snippet enrichment
        FH-->>AT: FinnhubComplete(result)
    end

    AT->>MW: analyze_company_workers(contexts)
    MW->>MS: stream generation requests (parallel workers)
    MS-->>MW: SSE token chunks
    MW-->>AT: StreamChunk + SectionComplete events
    AT-->>UI: AnalysisEvent(s) over mpsc channel
    UI->>UI: apply request_id guard, update App state, redraw
```

## State and safety mechanics

- `App` in `tui/state.rs` is the single UI state store (ticker, quote, candles, Finnhub datasets, statuses, errors, live 10s points).
- `analysis_request_id` prevents stale/background thread events from older requests from mutating current UI state.
- Yahoo live stream points are pruned to a rolling 10-second window.
- Finnhub dataset errors are preserved per dataset (`is_error`) so partial failures still render.

## Current UI surface vs internal pipeline

- Visible tabs are **Yahoo** and **Finnhub**.
- MLX worker output is still produced in the pipeline and tracked in app state, but the tab selector currently renders only Yahoo/Finnhub in the visible tab bar.
