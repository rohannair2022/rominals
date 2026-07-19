use crate::api::yahoo::Meta;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppTab {
    Yahoo,
    AlphaVantage,
    Ollama,
}

pub(crate) struct App {
    pub(crate) input: String,
    pub(crate) input_cursor_visible: bool,
    pub(crate) active_tab: AppTab,
    pub(crate) active_ticker: Option<String>,
    pub(crate) quote: Option<Meta>,
    pub(crate) alpha_fundamentals_snapshot: Option<String>,
    pub(crate) alpha_news_snapshot: Option<String>,
    pub(crate) analysis: Option<String>,
    pub(crate) analysis_loading: bool,
    pub(crate) analysis_request_id: u64,
    pub(crate) alpha_scroll: u16,
    pub(crate) ollama_scroll: u16,
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
            active_tab: AppTab::Yahoo,
            active_ticker: None,
            quote: None,
            alpha_fundamentals_snapshot: None,
            alpha_news_snapshot: None,
            analysis: None,
            analysis_loading: false,
            analysis_request_id: 0,
            alpha_scroll: 0,
            ollama_scroll: 0,
            analysis_error: None,
            comparison_ticker,
            error: None,
        }
    }
}

impl App {
    pub(crate) fn active_tab_index(&self) -> usize {
        match self.active_tab {
            AppTab::Yahoo => 0,
            AppTab::AlphaVantage => 1,
            AppTab::Ollama => 2,
        }
    }

    pub(crate) fn set_tab_index(&mut self, index: usize) {
        self.active_tab = match index {
            0 => AppTab::Yahoo,
            1 => AppTab::AlphaVantage,
            _ => AppTab::Ollama,
        };
    }

    pub(crate) fn next_tab(&mut self) {
        let next = (self.active_tab_index() + 1) % 3;
        self.set_tab_index(next);
    }

    pub(crate) fn prev_tab(&mut self) {
        let prev = (self.active_tab_index() + 2) % 3;
        self.set_tab_index(prev);
    }
}
