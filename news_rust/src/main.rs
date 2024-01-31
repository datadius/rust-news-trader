mod order_information;
mod position_list;
mod price_information;
mod symbol_information;

use order_information::OrderInformation;
use position_list::PositionList;
use price_information::PriceInformation;
use symbol_information::SymbolInformation;

use hex;
use hmac::Mac;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::env;
use std::error;
use std::io::Read;

fn construct_headers(payload: &str) -> HeaderMap {
    let api_key = env::var("testnet_bybit_order_key").expect("BYBIT_API_KEY not set");
    let api_secret = env::var("testnet_bybit_order_secret").expect("BYBIT_API_SECRET not set");
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let recv_window = "5000";
    let to_sign = format!(
        "{}{}{}{}",
        &current_timestamp, &api_key, &recv_window, payload
    );

    let signature = {
        type HmacSha256 = hmac::Hmac<sha2::Sha256>;
        let mut mac = HmacSha256::new_from_slice(&api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    let mut headers = HeaderMap::new();
    headers.insert("X-BAPI-API-KEY", HeaderValue::from_str(&api_key).unwrap());
    headers.insert("X-BAPI-SIGN", HeaderValue::from_str(&signature).unwrap());
    headers.insert(
        "X-BAPI-TIMESTAMP",
        HeaderValue::from_str(&current_timestamp).unwrap(),
    );
    headers.insert(
        "X-BAPI-RECV-WINDOW",
        HeaderValue::from_str(&recv_window).unwrap(),
    );
    headers.insert("Connection", HeaderValue::from_str("keep-alive").unwrap());
    headers.insert(
        "Content-Type",
        HeaderValue::from_str("application/json").unwrap(),
    );
    headers
}

fn get_order_qty(order_id: &str) -> Result<f32, Box<dyn error::Error>> {
    let params = format!("category=spot&order_id={}", order_id);
    let url = format!("https://api-testnet.bybit.com/v5/order/history?{}", params);
    let client = Client::new();
    let mut res = client
        .get(&url)
        .headers(construct_headers(&params))
        .send()?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let order_json: OrderInformation = serde_json::from_str(&body)?;

    println!("qty = {}", order_json.result.list[0].cumExecQty);
    println!("fee = {}", order_json.result.list[0].cumExecFee);

    let cum_exec_qty: f32 = order_json.result.list[0].cumExecQty.parse().unwrap();
    let cum_exec_fee: f32 = order_json.result.list[0].cumExecFee.parse().unwrap();

    Ok(cum_exec_qty - cum_exec_fee)
}

fn get_leverage(symbol: &str) -> Result<u8, Box<dyn error::Error>> {
    let params = format!("category=linear&symbol={}", symbol);
    let url = format!("https://api-testnet.bybit.com/v5/position/list?{}", params);
    let client = Client::new();
    let mut res = client
        .get(&url)
        .headers(construct_headers(&params))
        .send()?;

    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let leverage_json: PositionList = serde_json::from_str(&body)?;

    let value: u8 = leverage_json.result.list[0].leverage.parse().unwrap();

    Ok(value)
}

fn get_price(symbol: &str) -> Result<f32, Box<dyn error::Error>> {
    let url = format!(
        "https://api-testnet.bybit.com/v5/market/tickers?category=linear&symbol={}",
        symbol
    );
    let client = Client::new();
    let mut res = client.get(&url).send()?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let v: PriceInformation = serde_json::from_str(&body)?;

    let value: f32 = v.result.list[0].lastPrice.parse().unwrap();

    Ok(value)
}

fn get_symbol_information(symbol: &str) -> Result<f32, Box<dyn error::Error>> {
    let url = format!(
        "https://api-testnet.bybit.com/v5/market/instruments-info?category=linear&symbol={}",
        symbol
    );
    let mut res = reqwest::blocking::get(&url)?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let v: SymbolInformation = serde_json::from_str(&body)?;

    let value: f32 = v.result.list[0].lotSizeFilter.qtyStep.parse().unwrap();

    Ok(value)
}

fn market_buy_futures_position(symbol: &str, qty: f32) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    let payload = format!(
        r#"{{"category":"linear","symbol":"{}","side":"Buy","orderType":"Market","qty":"{}"}}"#,
        symbol, qty
    );

    println!("payload = {}", payload);

    let client = Client::new();
    let mut res = client
        .post(url)
        .headers(construct_headers(&payload))
        .body(payload)
        .send()?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let v: Value = serde_json::from_str(&body)?;
    println!("v = {}", v);

    Ok(())
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let qty_step = get_symbol_information("BTCUSDT")?;

    println!("qty_step = {}", qty_step);

    let leverage = get_leverage("BTCUSDT")?;

    println!("leverage = {}", leverage);

    let price = get_price("BTCUSDT")?;

    println!("price = {}", price);

    //let qty_ext = get_order_qty("85997568")?;

    //println!("qty = {}", qty_ext);

    market_buy_futures_position("BTCUSDT", 0.001)?;

    Ok(())
}
