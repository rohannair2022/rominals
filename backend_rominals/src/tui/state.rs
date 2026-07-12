use crate::api::yahoo::Meta;

#[derive(Default)]
pub(crate) struct App {
    pub(crate) input: String,
    pub(crate) active_ticker: Option<String>,
    pub(crate) quote: Option<Meta>,
    pub(crate) error: Option<String>,
}
