# Rominals

Rominals is now focused on a terminal user interface (TUI) for live ticker research.
Each ticker fetch now runs a stronger pipeline:
1. live quote from Yahoo Finance,
2. structured snapshot from Alpha Vantage fundamentals + NEWS_SENTIMENT (when configured),
3. parallel Ollama worker passes,
4. streamed final synthesis in the analysis panel.

The terminal now uses three tabs:
- **Yahoo**
- **Alpha Vantage** (fundamentals + news snapshot)
- **Ollama**

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
- Use `↑` / `↓` (or `PgUp` / `PgDn`) to scroll content on Alpha Vantage and Ollama tabs
- Press `Esc`, `Ctrl+C`, or `Ctrl+Q` to quit

## Research pipeline configuration

The app calls Ollama's local HTTP API (`/api/chat`) on every ticker fetch and runs a multi-pass analysis workflow.

Optional environment variables:

```bash
export ROMINALS_OLLAMA_HOST=http://127.0.0.1:11434
export ROMINALS_OLLAMA_MODEL=gpt-oss:120b-cloud
export ROMINALS_COMP_TICKER=MSFT
export ALPHAVANTAGE_API_KEY=your_key_here
```

- `ROMINALS_COMP_TICKER` is optional and enables "vs comp <ticker>" analysis context.
- `ALPHAVANTAGE_API_KEY` is optional; when set, the app injects structured fundamentals plus latest Alpha Vantage `NEWS_SENTIMENT` context before prompting Ollama.