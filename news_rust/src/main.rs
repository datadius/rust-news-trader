use error_chain::error_chain;
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        ValueError(serde_json::Error);
    }
}

#[derive(Deserialize)]
struct SymbolInformation {
    result: ListSymbols,
    retCode: i32,
    retExtInfo: Value,
    retMsg: String,
    time: i64,
}

#[derive(Deserialize)]
struct ListSymbols {
    category: String,
    list: Vec<Symbol>,
    nextPageCursor: String,
}

#[derive(Deserialize)]
struct Symbol {
    baseCoin: String,
    contractType: String,
    copyTrading: String,
    deliveryFeeRate: String,
    deliveryTime: String,
    fundingInterval: i32,
    launchTime: String,
    leverageFilter: LeverageFilter,
    lotSizeFilter: LotSizeFilter,
    priceFilter: PriceFilter,
    priceScale: String,
    quoteCoin: String,
    settleCoin: String,
    status: String,
    symbol: String,
    unifiedMarginTrade: bool,
}

#[derive(Deserialize)]
struct LeverageFilter {
    leverageStep: String,
    maxLeverage: String,
    minLeverage: String,
}

#[derive(Deserialize)]
struct LotSizeFilter {
    maxOrderQty: String,
    minOrderQty: String,
    postOnlyMaxOrderQty: String,
    qtyStep: String,
}

#[derive(Deserialize)]
struct PriceFilter {
    maxPrice: String,
    minPrice: String,
    tickSize: String,
}

fn get_symbol_information(symbol: &str) -> Result<()> {
    let url = format!(
        "https://api.bybit.com/v5/market/instruments-info?category=linear&symbol={}",
        symbol
    );
    let mut res = reqwest::blocking::get(&url)?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    //println!("body = {}", body);

    //let initial: Value = serde_json::from_str(&body)?;

    //println!("initial = {}", initial);

    let v: SymbolInformation = serde_json::from_str(&body)?;

    println!("v = {}", v.result.list[0].lotSizeFilter.qtyStep);

    Ok(())
}

fn main() -> Result<()> {
    get_symbol_information("BTCUSDT")?;

    Ok(())
}
