use crate::api::finnhub::{FinnhubSnapshot, finnhub_dataset_titles};
use crate::api::mlx::worker_section_titles;
use crate::api::yahoo::{Candle, CandleRange, Meta};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AppTab {
    Yahoo,
    Mlx,
    Finnhub,
}

pub(crate) struct MlxSectionState {
    pub(crate) title: String,
    pub(crate) content: Option<String>,
    pub(crate) scroll: u16,
}

pub(crate) struct FinnhubDatasetState {
    pub(crate) title: String,
    pub(crate) content: Option<String>,
    pub(crate) scroll: u16,
    pub(crate) is_error: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct YahooLivePoint {
    pub(crate) timestamp_ms: i64,
    pub(crate) price: f64,
}

pub(crate) struct App {
    pub(crate) input: String,
    pub(crate) input_cursor_visible: bool,
    pub(crate) active_tab: AppTab,
    pub(crate) active_ticker: Option<String>,
    pub(crate) quote: Option<Meta>,
    pub(crate) yahoo_candles: Vec<Candle>,
    pub(crate) yahoo_live_prices: Vec<YahooLivePoint>,
    pub(crate) yahoo_range: CandleRange,
    pub(crate) analysis_loading: bool,
    pub(crate) analysis_request_id: u64,
    pub(crate) analysis_error: Option<String>,
    pub(crate) mlx_status: Option<String>,
    pub(crate) mlx_sections: Vec<MlxSectionState>,
    pub(crate) active_mlx_section_index: usize,
    pub(crate) finnhub_status: Option<String>,
    pub(crate) finnhub_datasets: Vec<FinnhubDatasetState>,
    pub(crate) active_finnhub_dataset_index: usize,
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
            yahoo_candles: Vec::new(),
            yahoo_live_prices: Vec::new(),
            yahoo_range: CandleRange::default(),
            analysis_loading: false,
            analysis_request_id: 0,
            analysis_error: None,
            mlx_status: None,
            mlx_sections: worker_section_titles()
                .into_iter()
                .map(|title| MlxSectionState {
                    title,
                    content: None,
                    scroll: 0,
                })
                .collect(),
            active_mlx_section_index: 0,
            finnhub_status: None,
            finnhub_datasets: finnhub_dataset_titles()
                .into_iter()
                .map(|title| FinnhubDatasetState {
                    title,
                    content: None,
                    scroll: 0,
                    is_error: false,
                })
                .collect(),
            active_finnhub_dataset_index: 0,
            comparison_ticker,
            error: None,
        }
    }
}

impl App {
    const LIVE_WINDOW_MS: i64 = 10_000;

    pub(crate) fn active_tab_index(&self) -> usize {
        match self.active_tab {
            AppTab::Yahoo => 0,
            AppTab::Mlx | AppTab::Finnhub => 1,
        }
    }

    pub(crate) fn set_yahoo_range(&mut self, range: CandleRange) -> bool {
        if self.yahoo_range == range {
            return false;
        }
        self.yahoo_range = range;
        true
    }

    pub(crate) fn yahoo_range_index(&self) -> usize {
        self.yahoo_range.index()
    }

    pub(crate) fn next_yahoo_range(&mut self) -> bool {
        self.set_yahoo_range(self.yahoo_range.next())
    }

    pub(crate) fn prev_yahoo_range(&mut self) -> bool {
        self.set_yahoo_range(self.yahoo_range.prev())
    }

    pub(crate) fn push_yahoo_live_price(&mut self, timestamp_ms: i64, price: f64) {
        if !price.is_finite() {
            return;
        }

        self.yahoo_live_prices.push(YahooLivePoint {
            timestamp_ms,
            price,
        });
        self.prune_yahoo_live_prices(timestamp_ms);
    }

    pub(crate) fn prune_yahoo_live_prices(&mut self, now_ms: i64) {
        let cutoff = now_ms.saturating_sub(Self::LIVE_WINDOW_MS);
        self.yahoo_live_prices
            .retain(|point| point.timestamp_ms >= cutoff);
    }

    pub(crate) fn set_tab_index(&mut self, index: usize) {
        self.active_tab = match index {
            0 => AppTab::Yahoo,
            _ => AppTab::Finnhub,
        };
    }

    pub(crate) fn next_tab(&mut self) {
        self.set_tab_index((self.active_tab_index() + 1) % self.tab_count());
    }

    pub(crate) fn prev_tab(&mut self) {
        let current = self.active_tab_index();
        self.set_tab_index((current + self.tab_count() - 1) % self.tab_count());
    }

    pub(crate) fn reset_mlx_sections(&mut self) {
        self.active_mlx_section_index = 0;
        for section in &mut self.mlx_sections {
            section.content = None;
            section.scroll = 0;
        }
    }

    pub(crate) fn set_active_mlx_section_index(&mut self, index: usize) {
        if self.mlx_sections.is_empty() {
            self.active_mlx_section_index = 0;
            return;
        }

        self.active_mlx_section_index = index.min(self.mlx_sections.len() - 1);
    }

    pub(crate) fn next_mlx_section(&mut self) {
        if self.mlx_sections.is_empty() {
            return;
        }
        self.active_mlx_section_index =
            (self.active_mlx_section_index + 1) % self.mlx_sections.len();
    }

    pub(crate) fn prev_mlx_section(&mut self) {
        if self.mlx_sections.is_empty() {
            return;
        }
        if self.active_mlx_section_index == 0 {
            self.active_mlx_section_index = self.mlx_sections.len() - 1;
        } else {
            self.active_mlx_section_index -= 1;
        }
    }

    pub(crate) fn active_mlx_section_mut(&mut self) -> Option<&mut MlxSectionState> {
        self.mlx_sections.get_mut(self.active_mlx_section_index)
    }

    pub(crate) fn reset_finnhub_datasets(&mut self) {
        self.active_finnhub_dataset_index = 0;
        for dataset in &mut self.finnhub_datasets {
            dataset.content = None;
            dataset.scroll = 0;
            dataset.is_error = false;
        }
    }

    pub(crate) fn apply_finnhub_snapshot(&mut self, snapshot: FinnhubSnapshot) {
        self.reset_finnhub_datasets();
        for (slot, data) in self
            .finnhub_datasets
            .iter_mut()
            .zip(snapshot.datasets.into_iter())
        {
            slot.title = data.title;
            slot.content = Some(data.content);
            slot.is_error = data.is_error;
        }
    }

    pub(crate) fn set_active_finnhub_dataset_index(&mut self, index: usize) {
        if self.finnhub_datasets.is_empty() {
            self.active_finnhub_dataset_index = 0;
            return;
        }

        self.active_finnhub_dataset_index = index.min(self.finnhub_datasets.len() - 1);
    }

    pub(crate) fn next_finnhub_dataset(&mut self) {
        if self.finnhub_datasets.is_empty() {
            return;
        }
        self.active_finnhub_dataset_index =
            (self.active_finnhub_dataset_index + 1) % self.finnhub_datasets.len();
    }

    pub(crate) fn prev_finnhub_dataset(&mut self) {
        if self.finnhub_datasets.is_empty() {
            return;
        }
        if self.active_finnhub_dataset_index == 0 {
            self.active_finnhub_dataset_index = self.finnhub_datasets.len() - 1;
        } else {
            self.active_finnhub_dataset_index -= 1;
        }
    }

    pub(crate) fn active_finnhub_dataset_mut(&mut self) -> Option<&mut FinnhubDatasetState> {
        self.finnhub_datasets
            .get_mut(self.active_finnhub_dataset_index)
    }

    fn tab_count(&self) -> usize {
        2
    }
}
