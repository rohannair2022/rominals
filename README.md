# Rominals

Rominals is now focused on a terminal user interface (TUI) for viewing live Yahoo Finance quote data.
Each ticker fetch also triggers an Ollama company analysis request and displays it in the TUI.
Analysis is streamed into the panel as tokens arrive.

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
- Use `↑` / `↓` (or `PgUp` / `PgDn`) to scroll the analysis panel
- Press `Esc`, `Ctrl+C`, or `Ctrl+Q` to quit

## Ollama analysis configuration

The app calls Ollama's local HTTP API (`/api/chat`) on every ticker fetch using a company-analysis prompt.

Optional environment variables:

```bash
export ROMINALS_OLLAMA_HOST=http://127.0.0.1:11434
export ROMINALS_OLLAMA_MODEL=gpt-oss:120b-cloud
export ROMINALS_COMP_TICKER=MSFT
```

- `ROMINALS_COMP_TICKER` is optional and enables "vs comp <ticker>" analysis context.