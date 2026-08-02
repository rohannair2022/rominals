# Rominals

Rominals is now focused on a terminal user interface (TUI) for live ticker research.
Each ticker fetch now runs a stronger pipeline:
1. live quote from Yahoo Finance,
2. parallel MLX worker passes,
3. worker-by-worker output rendered in dedicated MLX sections.

The terminal now uses three tabs:
- **Yahoo**
- **MLX**
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
- Switch tabs with `Tab`, `Shift+Tab`, `←` / `→`, or `F1` / `F2` / `F3`
- In MLX/Finnhub tabs, switch sections with `[` / `]` or `1`-`9`
- Use `↑` / `↓` (or `PgUp` / `PgDn`) to scroll the active MLX/Finnhub section
- MLX section output streams live while workers generate (token flow is incremental)
- Press `Esc`, `Ctrl+C`, or `Ctrl+Q` to quit

## Research pipeline configuration

The app runs local `mlx-lm` generation workers on every ticker fetch and executes a two-section workflow (**Macro Outlook** + **Micro Outlook**) designed around standard top-down and bottom-up equity research framing.
The **Macro Outlook** worker now receives only macro-focused Finnhub context (general market news + market sentiment).
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
export ROMINALS_LINK_CONTEXT_TIMEOUT_SECS=6
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
- `market_sentiment`
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

In the **Finnhub** tab, `Market Sentiment` and `Insider Sentiment` now render in compact ASCII tables for easier terminal scanning.

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