mod api;
mod tui;

use std::error::Error;
use std::io;
use tui::controls::normalize_ticker;
use tui::run_tui;

fn usage_message(binary: &str) -> String {
    format!("Usage: {binary} [TICKER]   (e.g. AAPL, MSFT, KRKNF)")
}

fn parse_initial_ticker_from_args<I>(args: I) -> Result<Option<String>, io::Error>
where
    I: IntoIterator<Item = String>,
{
    let mut args = args.into_iter();
    let binary = args
        .next()
        .unwrap_or_else(|| "cargo run --release --".to_string());
    let initial_ticker = args.next();
    let has_extra_args = args.next().is_some();

    if has_extra_args {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            usage_message(&binary),
        ));
    }

    match initial_ticker {
        Some(raw) => normalize_ticker(&raw).map(Some).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{}\nTicker can only include letters, numbers, '.', '-', and '^'.",
                    usage_message(&binary)
                ),
            )
        }),
        None => Ok(None),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let initial_ticker = parse_initial_ticker_from_args(std::env::args())?;
    run_tui(initial_ticker)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_initial_ticker_allows_zero_or_one_arg() {
        let args = vec!["backend_rominals".to_string()];
        assert_eq!(parse_initial_ticker_from_args(args).unwrap(), None);

        let args = vec!["backend_rominals".to_string(), "msft".to_string()];
        assert_eq!(
            parse_initial_ticker_from_args(args).unwrap(),
            Some("MSFT".to_string())
        );
    }

    #[test]
    fn parse_initial_ticker_rejects_invalid_or_extra_args() {
        let invalid = vec!["backend_rominals".to_string(), "AAPL!".to_string()];
        assert!(parse_initial_ticker_from_args(invalid).is_err());

        let too_many = vec![
            "backend_rominals".to_string(),
            "AAPL".to_string(),
            "EXTRA".to_string(),
        ];
        assert!(parse_initial_ticker_from_args(too_many).is_err());
    }
}
