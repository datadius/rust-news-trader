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
    pub quantityPrecision: i8,
}
