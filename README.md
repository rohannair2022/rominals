# Rominals

Rominals is now focused on a terminal user interface (TUI) for live ticker research.
Each ticker fetch now runs a stronger pipeline:
1. live quote from Yahoo Finance,
2. parallel MLX worker passes,
3. worker-by-worker output generated in the background for internal LLM use.

The terminal now uses two visible tabs:
- **Yahoo**
- **Finnhub**

## Run the TUI

```bash
cd backend_rominals
cargo run --release
```

You can also start with an initial symbol:

```bash
cargo run --release -- AAPL
```

## Controls

- Type a ticker and press `Enter` to fetch data
- Press `Ctrl+R` to refresh the current symbol
- Yahoo quote + charts auto-refresh in the terminal every ~2 seconds (live stream view keeps a rolling 10s window)
- Switch tabs with `Tab`, `Shift+Tab`, `←` / `→`, or `F1` / `F2`
- In the Yahoo tab, cycle candle ranges with `[` / `]` or jump with `Ctrl+D` / `Ctrl+W` / `Ctrl+M` / `Ctrl+Y` / `Ctrl+A`
- In the Finnhub tab, switch datasets with `[` / `]` or `1`-`9`
- Use `↑` / `↓` (or `PgUp` / `PgDn`) to scroll the active Finnhub dataset
- Press `Esc`, `Ctrl+C`, or `Ctrl+Q` to quit

## Research pipeline configuration

The app runs local `mlx-lm` generation workers on every ticker fetch and executes a two-section workflow (**Macro Outlook** + **Micro Outlook**) designed around standard top-down and bottom-up equity research framing.
The LLM workflow uses a point-in-time Yahoo snapshot captured when you explicitly fetch (`Enter` / `Ctrl+R`) and does not rerun on the background Yahoo stream updates.
On each explicit fetch, Yahoo snapshot retrieval and full Finnhub dataset retrieval run concurrently in the background worker pipeline.
The **Macro Outlook** worker now receives only macro-focused Finnhub context (general market news).
The **Micro Outlook** worker receives Yahoo snapshot context plus micro-focused Finnhub context (profile/company news/peers/insiders/financials/filings/earnings/alternative data), with per-endpoint truncation to protect prompt size.
If Finnhub datasets include links, the app also fetches a capped subset of those URLs and appends compact scraped snippets to prompt context (bounded by source count + character limits).
By default it runs up to two workers concurrently (one per section).
When no initial ticker is passed, the app preloads the MLX model at startup so the first fetch has less cold-start overhead.

Install `mlx-lm` first:

```bash
python3 -m pip install mlx-lm
```

Optional environment variables:

```bash
export ROMINALS_MLX_PYTHON_BIN=python3
export ROMINALS_MLX_MODEL=mlx-community/Qwen3.5-4B-MLX-4bit
export ROMINALS_MLX_MAX_TOKENS=500
export ROMINALS_MLX_TEMPERATURE=0.2
export ROMINALS_MLX_PARALLEL_WORKERS=2
export ROMINALS_MLX_ENABLE_THINKING=false
export ROMINALS_FINNHUB_API_KEY=your_finnhub_key_here
export ROMINALS_COMP_TICKER=MSFT
export ROMINALS_LINK_CONTEXT_ENABLED=true
export ROMINALS_LINK_CONTEXT_MAX_URLS=2
export ROMINALS_LINK_CONTEXT_MAX_CHARS_PER_URL=700
export ROMINALS_LINK_CONTEXT_MAX_TOTAL_CHARS=1400
export ROMINALS_LINK_CONTEXT_MAX_FETCH_BYTES=90000
```

- `ROMINALS_COMP_TICKER` is optional and enables "vs comp <ticker>" analysis context.
- `ROMINALS_MLX_ENABLE_THINKING` defaults to `false`; set it to `true` to include reasoning output.
- `ROMINALS_FINNHUB_API_KEY` (or `FINNHUB_API_KEY`) enables the Finnhub service client.
- `ROMINALS_LINK_CONTEXT_*` controls optional URL scraping caps for LLM context enrichment.

## Finnhub service endpoints

`backend_rominals/src/api/finnhub.rs` exposes endpoint helpers for:
- `stock_profile`
- `news`
- `company_news`
- `peers`
- `insider_transactions`
- `insider_sentiments`
- `financials_reported`
- `sec_filings`
- `earnings_surprises`
- `earnings`
- `uspto_patents`
- `stock_lobbying`
- `stock_usa_spending`

In the **Finnhub** tab, `Insider Sentiment` renders in a compact ASCII table for easier terminal scanning.

For isolated Python dependencies, use a dedicated venv and point the runtime to it:

```bash
python3 -m venv .venv-mlx
.venv-mlx/bin/python -m pip install mlx-lm
export ROMINALS_MLX_PYTHON_BIN="$(pwd)/.venv-mlx/bin/python"
```

## Live token streaming test (MLX)

To watch tokens stream directly in your terminal (instead of waiting for full completion), run:

```bash
.venv-mlx/bin/python backend_rominals/scripts/mlx_stream_test.py \
  --model mlx-community/Qwen3.5-4B-MLX-4bit \
  --prompt "Write a concise AAPL market thesis with one risk and one catalyst." \
  --max-tokens 200
```