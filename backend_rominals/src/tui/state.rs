use crate::api::yahoo::Meta;

pub(crate) struct App {
    pub(crate) input: String,
    pub(crate) input_cursor_visible: bool,
    pub(crate) active_ticker: Option<String>,
    pub(crate) quote: Option<Meta>,
    pub(crate) analysis: Option<String>,
    pub(crate) analysis_loading: bool,
    pub(crate) analysis_request_id: u64,
    pub(crate) analysis_scroll: u16,
    pub(crate) analysis_error: Option<String>,
    pub(crate) comparison_ticker: Option<String>,
    pub(crate) error: Option<String>,
}

impl Default for App {
    fn default() -> Self {
        let comparison_ticker = std::env::var("ROMINALS_COMP_TICKER")
            .ok()
            .and_then(|ticker| {
                let normalized = ticker.trim().to_uppercase();
                if normalized.is_empty() {
                    None
                } else {
                    Some(normalized)
                }
            });

        Self {
            input: String::new(),
            input_cursor_visible: true,
            active_ticker: None,
            quote: None,
            analysis: None,
            analysis_loading: false,
            analysis_request_id: 0,
            analysis_scroll: 0,
            analysis_error: None,
            comparison_ticker,
            error: None,
        }
    }
}
