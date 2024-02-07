use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct ExchangeInfo {
    pub symbols: Vec<Symbol>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub filters: Vec<Filter>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct Filter{
    pub filterType: String,
    pub stepSize: String,
}
