use serde::Serialize;
use serde_json::Value;
use std::error::Error;
use std::io;
use std::time::Duration;

const BASE_URL: &str = "https://www.alphavantage.co/query";

#[derive(Debug, Serialize)]
struct SnapshotContext {
    symbol: String,
    overview: OverviewSnapshot,
    financials: FinancialSnapshot,
    news: NewsSnapshot,
    warnings: Vec<String>,
}

#[derive(Debug, Serialize, Default)]
struct OverviewSnapshot {
    name: Option<String>,
    exchange: Option<String>,
    sector: Option<String>,
    industry: Option<String>,
    country: Option<String>,
    market_cap: Option<f64>,
    pe_ratio: Option<f64>,
    peg_ratio: Option<f64>,
    price_to_sales_ttm: Option<f64>,
    ev_to_ebitda: Option<f64>,
    profit_margin: Option<f64>,
    operating_margin_ttm: Option<f64>,
    revenue_ttm: Option<f64>,
    gross_profit_ttm: Option<f64>,
    quarterly_revenue_growth_yoy: Option<f64>,
    quarterly_earnings_growth_yoy: Option<f64>,
    analyst_target_price: Option<f64>,
    beta: Option<f64>,
}

#[derive(Debug, Serialize, Default)]
struct FinancialSnapshot {
    latest_annual_revenue: Option<f64>,
    latest_annual_gross_profit: Option<f64>,
    latest_annual_net_income: Option<f64>,
    latest_operating_cash_flow: Option<f64>,
    latest_capex: Option<f64>,
    latest_free_cash_flow: Option<f64>,
    total_cash_and_equivalents: Option<f64>,
    total_debt: Option<f64>,
    current_ratio: Option<f64>,
}

#[derive(Debug, Serialize, Default)]
struct NewsSnapshot {
    articles: Vec<NewsArticle>,
}

#[derive(Debug, Serialize)]
struct NewsArticle {
    title: Option<String>,
    source: Option<String>,
    time_published: Option<String>,
    overall_sentiment_score: Option<f64>,
    overall_sentiment_label: Option<String>,
    summary: Option<String>,
}

pub fn fetch_snapshot_context(ticker: &str) -> Result<Option<String>, Box<dyn Error>> {
    let api_key = match std::env::var("ALPHAVANTAGE_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => return Ok(None),
    };

    let client = reqwest::blocking::Client::builder()
        .user_agent("rominals-alpha-vantage/0.1")
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(5))
        .build()?;

    let mut warnings = Vec::new();

    let overview = fetch_json_optional(
        &client,
        "OVERVIEW",
        &[("symbol", ticker)],
        &api_key,
        &mut warnings,
    );
    let income_statement = fetch_json_optional(
        &client,
        "INCOME_STATEMENT",
        &[("symbol", ticker)],
        &api_key,
        &mut warnings,
    );
    let cash_flow = fetch_json_optional(
        &client,
        "CASH_FLOW",
        &[("symbol", ticker)],
        &api_key,
        &mut warnings,
    );
    let balance_sheet = fetch_json_optional(
        &client,
        "BALANCE_SHEET",
        &[("symbol", ticker)],
        &api_key,
        &mut warnings,
    );
    let news_sentiment = fetch_json_optional(
        &client,
        "NEWS_SENTIMENT",
        &[("tickers", ticker), ("sort", "LATEST"), ("limit", "10")],
        &api_key,
        &mut warnings,
    );

    if overview.is_none()
        && income_statement.is_none()
        && cash_flow.is_none()
        && balance_sheet.is_none()
        && news_sentiment.is_none()
    {
        return Err(io::Error::other(format!(
            "Alpha Vantage returned no usable context for {ticker}. warnings={warnings:?}"
        ))
        .into());
    }

    let context = SnapshotContext {
        symbol: ticker.to_string(),
        overview: parse_overview(overview.as_ref()),
        financials: parse_financials(
            income_statement.as_ref(),
            cash_flow.as_ref(),
            balance_sheet.as_ref(),
        ),
        news: parse_news(news_sentiment.as_ref()),
        warnings,
    };

    Ok(Some(serde_json::to_string_pretty(&context)?))
}

fn fetch_json_optional(
    client: &reqwest::blocking::Client,
    function: &str,
    params: &[(&str, &str)],
    api_key: &str,
    warnings: &mut Vec<String>,
) -> Option<Value> {
    match fetch_json(client, function, params, api_key) {
        Ok(value) => Some(value),
        Err(err) => {
            warnings.push(format!("{function}: {err}"));
            None
        }
    }
}

fn fetch_json(
    client: &reqwest::blocking::Client,
    function: &str,
    params: &[(&str, &str)],
    api_key: &str,
) -> Result<Value, io::Error> {
    let mut query: Vec<(&str, String)> = vec![
        ("function", function.to_string()),
        ("apikey", api_key.to_string()),
    ];
    for (key, value) in params {
        query.push((*key, (*value).to_string()));
    }

    let resp = client
        .get(BASE_URL)
        .query(&query)
        .send()
        .map_err(|err| io::Error::other(format!("Alpha Vantage request failed: {err}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp
            .text()
            .unwrap_or_else(|err| format!("<failed to read error body: {err}>"));
        return Err(io::Error::other(format!(
            "Alpha Vantage HTTP {status} for {function}: {body}"
        )));
    }

    let body: Value = resp
        .json()
        .map_err(|err| io::Error::other(format!("Alpha Vantage JSON decode failed: {err}")))?;

    if let Some(note) = body.get("Note").and_then(Value::as_str) {
        return Err(io::Error::other(format!("rate limit hit: {note}")));
    }
    if let Some(info) = body.get("Information").and_then(Value::as_str) {
        return Err(io::Error::other(format!("info response: {info}")));
    }
    if let Some(err) = body.get("Error Message").and_then(Value::as_str) {
        return Err(io::Error::other(format!("api error: {err}")));
    }

    Ok(body)
}

fn parse_overview(overview: Option<&Value>) -> OverviewSnapshot {
    let Some(overview) = overview else {
        return OverviewSnapshot::default();
    };

    OverviewSnapshot {
        name: value_string(overview, "Name"),
        exchange: value_string(overview, "Exchange"),
        sector: value_string(overview, "Sector"),
        industry: value_string(overview, "Industry"),
        country: value_string(overview, "Country"),
        market_cap: value_f64(overview, "MarketCapitalization"),
        pe_ratio: value_f64(overview, "PERatio"),
        peg_ratio: value_f64(overview, "PEGRatio"),
        price_to_sales_ttm: value_f64(overview, "PriceToSalesRatioTTM"),
        ev_to_ebitda: value_f64(overview, "EVToEBITDA"),
        profit_margin: value_f64(overview, "ProfitMargin"),
        operating_margin_ttm: value_f64(overview, "OperatingMarginTTM"),
        revenue_ttm: value_f64(overview, "RevenueTTM"),
        gross_profit_ttm: value_f64(overview, "GrossProfitTTM"),
        quarterly_revenue_growth_yoy: value_f64(overview, "QuarterlyRevenueGrowthYOY"),
        quarterly_earnings_growth_yoy: value_f64(overview, "QuarterlyEarningsGrowthYOY"),
        analyst_target_price: value_f64(overview, "AnalystTargetPrice"),
        beta: value_f64(overview, "Beta"),
    }
}

fn parse_financials(
    income: Option<&Value>,
    cash_flow: Option<&Value>,
    balance_sheet: Option<&Value>,
) -> FinancialSnapshot {
    let income_latest = first_report(income, "annualReports");
    let cash_latest = first_report(cash_flow, "annualReports");
    let balance_latest = first_report(balance_sheet, "annualReports");

    let operating_cash_flow = report_f64(cash_latest, "operatingCashflow");
    let capex = report_f64(cash_latest, "capitalExpenditures");

    FinancialSnapshot {
        latest_annual_revenue: report_f64(income_latest, "totalRevenue"),
        latest_annual_gross_profit: report_f64(income_latest, "grossProfit"),
        latest_annual_net_income: report_f64(income_latest, "netIncome"),
        latest_operating_cash_flow: operating_cash_flow,
        latest_capex: capex,
        latest_free_cash_flow: match (operating_cash_flow, capex) {
            (Some(ocf), Some(cap)) => Some(ocf - cap.abs()),
            _ => None,
        },
        total_cash_and_equivalents: report_f64(
            balance_latest,
            "cashAndCashEquivalentsAtCarryingValue",
        ),
        total_debt: report_f64(balance_latest, "shortLongTermDebtTotal"),
        current_ratio: report_f64(balance_latest, "currentRatio"),
    }
}

fn parse_news(news: Option<&Value>) -> NewsSnapshot {
    let Some(news) = news else {
        return NewsSnapshot::default();
    };

    let Some(feed) = news.get("feed").and_then(Value::as_array) else {
        return NewsSnapshot::default();
    };

    let articles = feed
        .iter()
        .take(10)
        .map(|item| NewsArticle {
            title: value_string(item, "title"),
            source: value_string(item, "source"),
            time_published: value_string(item, "time_published"),
            overall_sentiment_score: value_f64(item, "overall_sentiment_score"),
            overall_sentiment_label: value_string(item, "overall_sentiment_label"),
            summary: value_string(item, "summary"),
        })
        .collect();

    NewsSnapshot { articles }
}

fn first_report<'a>(data: Option<&'a Value>, key: &str) -> Option<&'a Value> {
    data.and_then(|value| value.get(key))
        .and_then(Value::as_array)
        .and_then(|reports| reports.first())
}

fn report_f64(report: Option<&Value>, key: &str) -> Option<f64> {
    report
        .and_then(|row| row.get(key))
        .and_then(parse_value_f64)
}

fn value_string(row: &Value, key: &str) -> Option<String> {
    row.get(key).and_then(Value::as_str).and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn value_f64(row: &Value, key: &str) -> Option<f64> {
    row.get(key).and_then(parse_value_f64)
}

fn parse_value_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Number(number) => number.as_f64(),
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
                None
            } else {
                trimmed.parse::<f64>().ok()
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_value_f64_handles_numeric_string() {
        let value = Value::String("123.45".to_string());
        assert_eq!(parse_value_f64(&value), Some(123.45));
    }

    #[test]
    fn parse_news_extracts_article_fields() {
        let sample = serde_json::json!({
            "feed": [{
                "title": "Sample headline",
                "source": "Reuters",
                "time_published": "20260719T120000",
                "overall_sentiment_score": "0.42",
                "overall_sentiment_label": "Bullish",
                "summary": "Sample summary"
            }]
        });

        let news = parse_news(Some(&sample));
        assert_eq!(news.articles.len(), 1);
        assert_eq!(news.articles[0].title.as_deref(), Some("Sample headline"));
        assert_eq!(news.articles[0].overall_sentiment_score, Some(0.42));
    }
}
