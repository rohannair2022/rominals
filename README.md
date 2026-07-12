# Rominals

Rominals is now focused on a terminal user interface (TUI) for viewing live Yahoo Finance quote data.

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
- Press `r` to refresh the current symbol
- Press `q` to quit