mod position_list;
mod symbol_information;

use position_list::PositionList;
use symbol_information::SymbolInformation;

use error_chain::error_chain;
use hex;
use hmac::Mac;
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;
use std::env;
use std::io::Read;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
        ValueError(serde_json::Error);
    }
}

fn construct_headers(payload: &str) -> HeaderMap {
    let api_key = env::var("bybit_order_key").expect("BYBIT_API_KEY not set");
    let api_secret = env::var("bybit_order_secret").expect("BYBIT_API_SECRET not set");
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

fn get_leverage(symbol: &str) -> Result<()> {
    let params = format!("category=linear&symbol={}", symbol);
    let url = format!("https://api.bybit.com/v5/position/list?{}", params);
    let client = Client::new();
    let mut res = client
        .get(&url)
        .headers(construct_headers(&params))
        .send()?;

    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let leverage_json: PositionList = serde_json::from_str(&body)?;

    println!("leverage_json = {}", leverage_json.result.list[0].leverage);

    Ok(())
}
fn get_symbol_information(symbol: &str) -> Result<()> {
    let url = format!(
        "https://api.bybit.com/v5/market/instruments-info?category=linear&symbol={}",
        symbol
    );
    let mut res = reqwest::blocking::get(&url)?;
    let mut body = String::new();
    res.read_to_string(&mut body)?;

    let v: SymbolInformation = serde_json::from_str(&body)?;

    println!("v = {}", v.result.list[0].lotSizeFilter.qtyStep);

    Ok(())
}

fn main() -> Result<()> {
    get_symbol_information("BTCUSDT")?;

    get_leverage("BTCUSDT")?;

    Ok(())
}
