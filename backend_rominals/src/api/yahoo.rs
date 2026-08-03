use serde::Deserialize;
use std::error::Error;
use std::io;

#[derive(Debug, Deserialize)]
struct ChartResponse {
    chart: Chart,
}

#[derive(Debug, Deserialize)]
struct Chart {
    result: Option<Vec<ChartResult>>,
    error: Option<ChartApiError>,
}

#[derive(Debug, Deserialize)]
struct ChartResult {
    meta: Meta,
    timestamp: Option<Vec<i64>>,
    indicators: Option<Indicators>,
}

#[derive(Debug, Deserialize)]
struct Indicators {
    quote: Option<Vec<QuoteSeries>>,
}

#[derive(Debug, Deserialize)]
struct QuoteSeries {
    open: Option<Vec<Option<f64>>>,
    high: Option<Vec<Option<f64>>>,
    low: Option<Vec<Option<f64>>>,
    close: Option<Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct ChartApiError {
    code: Option<String>,
    description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub symbol: String,
    pub currency: Option<String>,
    pub full_exchange_name: Option<String>,
    pub long_name: Option<String>,
    pub short_name: Option<String>,
    pub regular_market_price: Option<f64>,
    pub chart_previous_close: Option<f64>,
    pub regular_market_day_high: Option<f64>,
    pub regular_market_day_low: Option<f64>,
    pub regular_market_volume: Option<i64>,
    pub fifty_two_week_high: Option<f64>,
    pub fifty_two_week_low: Option<f64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandleRange {
    Day,
    Week,
    Month,
    Year,
    All,
}

impl CandleRange {
    pub const ORDERED: [Self; 5] = [Self::Day, Self::Week, Self::Month, Self::Year, Self::All];

    pub fn index(self) -> usize {
        match self {
            Self::Day => 0,
            Self::Week => 1,
            Self::Month => 2,
            Self::Year => 3,
            Self::All => 4,
        }
    }

    pub fn from_index(index: usize) -> Self {
        Self::ORDERED.get(index).copied().unwrap_or(Self::Day)
    }

    pub fn next(self) -> Self {
        Self::from_index((self.index() + 1) % Self::ORDERED.len())
    }

    pub fn prev(self) -> Self {
        Self::from_index((self.index() + Self::ORDERED.len() - 1) % Self::ORDERED.len())
    }

    pub fn tab_label(self) -> &'static str {
        match self {
            Self::Day => "D",
            Self::Week => "W",
            Self::Month => "M",
            Self::Year => "Y",
            Self::All => "ALL",
        }
    }

    pub fn title_label(self) -> &'static str {
        match self {
            Self::Day => "Daily",
            Self::Week => "Weekly",
            Self::Month => "Monthly",
            Self::Year => "Yearly",
            Self::All => "All",
        }
    }

    fn yahoo_query(self) -> (&'static str, &'static str) {
        match self {
            Self::Day => ("1d", "1m"),
            Self::Week => ("1wk", "5m"),
            Self::Month => ("1mo", "15m"),
            Self::Year => ("1y", "1d"),
            Self::All => ("max", "1wk"),
        }
    }
}

impl Default for CandleRange {
    fn default() -> Self {
        Self::Day
    }
}

#[derive(Debug, Clone)]
pub struct QuoteSnapshot {
    pub meta: Meta,
    pub candles: Vec<Candle>,
}

fn format_opt_f64(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:.2}"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_opt_pct(value: Option<f64>) -> String {
    value
        .map(|v| format!("{v:+.2}%"))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_opt_i64(value: Option<i64>) -> String {
    value
        .map(|v| v.to_string())
        .unwrap_or_else(|| "n/a".to_string())
}

fn safe_ratio(numerator: f64, denominator: f64) -> Option<f64> {
    if denominator.abs() <= f64::EPSILON {
        None
    } else {
        Some(numerator / denominator)
    }
}

fn percent_change(current: Option<f64>, baseline: Option<f64>) -> Option<f64> {
    let (current, baseline) = (current?, baseline?);
    safe_ratio(current - baseline, baseline).map(|value| value * 100.0)
}

fn span_percent(low: Option<f64>, high: Option<f64>, anchor: Option<f64>) -> Option<f64> {
    let (low, high, anchor) = (low?, high?, anchor?);
    safe_ratio(high - low, anchor).map(|value| value * 100.0)
}

fn range_position_percent(low: Option<f64>, high: Option<f64>, value: Option<f64>) -> Option<f64> {
    let (low, high, value) = (low?, high?, value?);
    safe_ratio(value - low, high - low).map(|ratio| ratio * 100.0)
}

fn distance_to_high_percent(value: Option<f64>, high: Option<f64>) -> Option<f64> {
    let (value, high) = (value?, high?);
    safe_ratio(high - value, high).map(|ratio| ratio * 100.0)
}

fn distance_to_low_percent(value: Option<f64>, low: Option<f64>) -> Option<f64> {
    let (value, low) = (value?, low?);
    safe_ratio(value - low, low).map(|ratio| ratio * 100.0)
}

fn momentum_regime(day_change_pct: Option<f64>) -> &'static str {
    match day_change_pct {
        Some(value) if value >= 3.0 => "strong bullish day move",
        Some(value) if value >= 1.0 => "mild bullish day move",
        Some(value) if value <= -3.0 => "strong bearish day move",
        Some(value) if value <= -1.0 => "mild bearish day move",
        Some(_) => "flat/neutral day move",
        None => "unknown day move",
    }
}

fn yearly_range_regime(position_pct: Option<f64>) -> &'static str {
    match position_pct {
        Some(value) if value >= 85.0 => "near 52-week highs",
        Some(value) if value <= 15.0 => "near 52-week lows",
        Some(value) if value > 60.0 => "upper half of 52-week range",
        Some(value) if value < 40.0 => "lower half of 52-week range",
        Some(_) => "middle of 52-week range",
        None => "unknown yearly range position",
    }
}

fn liquidity_regime(volume: Option<i64>) -> &'static str {
    match volume {
        Some(v) if v >= 20_000_000 => "high trading activity",
        Some(v) if v >= 5_000_000 => "normal trading activity",
        Some(_) => "light trading activity",
        None => "unknown trading activity",
    }
}

pub fn build_analysis_context(meta: &Meta) -> String {
    let display_name = meta
        .long_name
        .as_deref()
        .or(meta.short_name.as_deref())
        .unwrap_or(&meta.symbol);

    let day_change_pct = percent_change(meta.regular_market_price, meta.chart_previous_close);
    let intraday_span_pct = span_percent(
        meta.regular_market_day_low,
        meta.regular_market_day_high,
        meta.regular_market_price,
    );
    let yearly_position_pct = range_position_percent(
        meta.fifty_two_week_low,
        meta.fifty_two_week_high,
        meta.regular_market_price,
    );
    let dist_to_52w_high_pct =
        distance_to_high_percent(meta.regular_market_price, meta.fifty_two_week_high);
    let dist_to_52w_low_pct =
        distance_to_low_percent(meta.regular_market_price, meta.fifty_two_week_low);

    let exchange = meta
        .full_exchange_name
        .clone()
        .unwrap_or_else(|| "n/a".to_string());
    let currency = meta.currency.clone().unwrap_or_else(|| "n/a".to_string());

    format!(
        "Yahoo snapshot for {symbol} ({name})\n\
Exchange: {exchange} | Currency: {currency}\n\
Price: {price} | Prev close: {prev_close} | Day change: {day_change}\n\
Day range: {day_low} - {day_high} | Intraday span: {intraday_span}\n\
52-week range: {wk_low} - {wk_high} | Position in 52-week band: {yearly_position}\n\
Distance to 52-week high: {dist_high} | Distance to 52-week low: {dist_low}\n\
Volume: {volume}\n\
\n\
Inference cues (soft assumptions, not hard facts):\n\
- Momentum regime: {momentum}\n\
- Yearly range regime: {range_regime}\n\
- Liquidity regime: {liquidity}\n\
- If valuation metrics like P/S, EV, EV/EBITDA are missing, explicitly state unknown instead of inferring.",
        symbol = meta.symbol,
        name = display_name,
        exchange = exchange,
        currency = currency,
        price = format_opt_f64(meta.regular_market_price),
        prev_close = format_opt_f64(meta.chart_previous_close),
        day_change = format_opt_pct(day_change_pct),
        day_low = format_opt_f64(meta.regular_market_day_low),
        day_high = format_opt_f64(meta.regular_market_day_high),
        intraday_span = format_opt_pct(intraday_span_pct),
        wk_low = format_opt_f64(meta.fifty_two_week_low),
        wk_high = format_opt_f64(meta.fifty_two_week_high),
        yearly_position = format_opt_pct(yearly_position_pct),
        dist_high = format_opt_pct(dist_to_52w_high_pct),
        dist_low = format_opt_pct(dist_to_52w_low_pct),
        volume = format_opt_i64(meta.regular_market_volume),
        momentum = momentum_regime(day_change_pct),
        range_regime = yearly_range_regime(yearly_position_pct),
        liquidity = liquidity_regime(meta.regular_market_volume),
    )
}

pub fn fetch_quote_snapshot(
    ticker: &str,
    candle_range: CandleRange,
) -> Result<QuoteSnapshot, Box<dyn Error>> {
    let url = format!("https://query1.finance.yahoo.com/v8/finance/chart/{ticker}");
    let (range, interval) = candle_range.yahoo_query();

    let client = reqwest::blocking::Client::builder()
        .user_agent("Mozilla/5.0 (rust-yfinance/0.1)")
        .build()?;

    let resp = client
        .get(&url)
        .query(&[("range", range), ("interval", interval)])
        .send()?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp
            .text()
            .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
        return Err(io::Error::other(format!(
            "HTTP {status} from Yahoo for {ticker}: {error_body}"
        ))
        .into());
    }

    let body: ChartResponse = resp.json()?;
    extract_snapshot(body, ticker).map_err(Into::into)
}

fn extract_snapshot(body: ChartResponse, ticker: &str) -> Result<QuoteSnapshot, io::Error> {
    let Chart { result, error } = body.chart;

    if let Some(api_error) = error {
        let code = api_error.code.unwrap_or_else(|| "unknown".to_string());
        let description = api_error
            .description
            .unwrap_or_else(|| "No description provided".to_string());
        return Err(io::Error::other(format!(
            "Yahoo API error for {ticker} [{code}]: {description}"
        )));
    }

    let first_result = result
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Yahoo response did not include quote results for {ticker}"),
            )
        })?
        .into_iter()
        .next()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Yahoo returned an empty result list for {ticker}"),
            )
        })?;

    let candles = extract_candles(&first_result);

    Ok(QuoteSnapshot {
        meta: first_result.meta,
        candles,
    })
}

fn extract_candles(result: &ChartResult) -> Vec<Candle> {
    let Some(timestamps) = result.timestamp.as_ref() else {
        return Vec::new();
    };
    let Some(series) = result
        .indicators
        .as_ref()
        .and_then(|indicators| indicators.quote.as_ref())
        .and_then(|quotes| quotes.first())
    else {
        return Vec::new();
    };

    let mut candles = Vec::with_capacity(timestamps.len());

    for (index, timestamp) in timestamps.iter().copied().enumerate() {
        let Some(open) = series_value(&series.open, index) else {
            continue;
        };
        let Some(high) = series_value(&series.high, index) else {
            continue;
        };
        let Some(low) = series_value(&series.low, index) else {
            continue;
        };
        let Some(close) = series_value(&series.close, index) else {
            continue;
        };

        if high < low {
            continue;
        }

        candles.push(Candle {
            timestamp,
            open,
            high,
            low,
            close,
        });
    }

    candles
}

fn series_value(series: &Option<Vec<Option<f64>>>, index: usize) -> Option<f64> {
    let value = series.as_ref()?.get(index).copied().flatten()?;
    if value.is_finite() { Some(value) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta(symbol: &str) -> Meta {
        Meta {
            symbol: symbol.to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn meta_default_sets_empty_symbol_and_none_optionals() {
        let meta = Meta::default();
        assert_eq!(meta.symbol, "");
        assert!(meta.long_name.is_none());
        assert!(meta.regular_market_price.is_none());
    }

    #[test]
    fn extract_meta_returns_first_result_meta() {
        let body = ChartResponse {
            chart: Chart {
                result: Some(vec![ChartResult {
                    meta: sample_meta("AAPL"),
                    timestamp: None,
                    indicators: None,
                }]),
                error: None,
            },
        };

        let snapshot = extract_snapshot(body, "AAPL").unwrap();
        assert_eq!(snapshot.meta.symbol, "AAPL");
        assert!(snapshot.candles.is_empty());
    }

    #[test]
    fn extract_meta_returns_api_error_details() {
        let body = ChartResponse {
            chart: Chart {
                result: None,
                error: Some(ChartApiError {
                    code: Some("Not Found".to_string()),
                    description: Some("No data found".to_string()),
                }),
            },
        };

        let err = extract_snapshot(body, "INVALID").unwrap_err();
        assert_eq!(
            err.to_string(),
            "Yahoo API error for INVALID [Not Found]: No data found"
        );
    }

    #[test]
    fn extract_snapshot_parses_ohlc_candles() {
        let body = ChartResponse {
            chart: Chart {
                result: Some(vec![ChartResult {
                    meta: sample_meta("AAPL"),
                    timestamp: Some(vec![1, 2, 3]),
                    indicators: Some(Indicators {
                        quote: Some(vec![QuoteSeries {
                            open: Some(vec![Some(10.0), Some(12.0), None]),
                            high: Some(vec![Some(11.0), Some(13.0), Some(15.0)]),
                            low: Some(vec![Some(9.5), Some(11.5), Some(14.0)]),
                            close: Some(vec![Some(10.5), Some(12.5), Some(14.5)]),
                        }]),
                    }),
                }]),
                error: None,
            },
        };

        let snapshot = extract_snapshot(body, "AAPL").unwrap();
        assert_eq!(snapshot.candles.len(), 2);
        assert_eq!(
            snapshot.candles[0],
            Candle {
                timestamp: 1,
                open: 10.0,
                high: 11.0,
                low: 9.5,
                close: 10.5
            }
        );
    }

    #[test]
    fn build_analysis_context_contains_key_sections() {
        let meta = Meta {
            symbol: "AAPL".to_string(),
            long_name: Some("Apple Inc.".to_string()),
            full_exchange_name: Some("NasdaqGS".to_string()),
            currency: Some("USD".to_string()),
            regular_market_price: Some(200.0),
            chart_previous_close: Some(194.0),
            regular_market_day_high: Some(201.0),
            regular_market_day_low: Some(196.0),
            regular_market_volume: Some(22_000_000),
            fifty_two_week_high: Some(220.0),
            fifty_two_week_low: Some(150.0),
            ..Default::default()
        };

        let context = build_analysis_context(&meta);
        assert!(context.contains("Yahoo snapshot for AAPL"));
        assert!(context.contains("Inference cues (soft assumptions, not hard facts)"));
        assert!(context.contains("Momentum regime: strong bullish day move"));
    }
}
