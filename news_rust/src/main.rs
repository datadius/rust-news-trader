mod order_information;
mod position_list;
mod price_information;
mod symbol_information;
mod tree_response;

use order_information::OrderInformation;
use position_list::PositionList;
use price_information::PriceInformation;
use symbol_information::SymbolInformation;
use tree_response::TreeResponse;

use env_logger;
use futures::{SinkExt, StreamExt};
use hex;
use hmac::Mac;
use log::{debug, error, info};
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::Client;
use serde_json::Value;
use std::env;
use std::error;
use std::io::Read;
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message,
    tungstenite::{Error, Result},
};
use url::Url;

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

async fn get_order_qty(client: Client, order_id: &str) -> Result<f32, Box<dyn error::Error>> {
    let params = format!("category=spot&order_id={}", order_id);
    let url = format!("https://api-testnet.bybit.com/v5/order/history?{}", params);
    let res = client
        .get(&url)
        .headers(construct_headers(&params))
        .send()
        .await?;
    let body = res.text().await?;

    let order_json: OrderInformation = serde_json::from_str(&body)?;

    info!("qty = {}", order_json.result.list[0].cumExecQty);
    info!("fee = {}", order_json.result.list[0].cumExecFee);

    let cum_exec_qty: f32 = order_json.result.list[0].cumExecQty.parse().unwrap();
    let cum_exec_fee: f32 = order_json.result.list[0].cumExecFee.parse().unwrap();

    Ok(cum_exec_qty - cum_exec_fee)
}

async fn get_leverage(client: Client, symbol: &str) -> Result<f32, Box<dyn error::Error>> {
    let params = format!("category=linear&symbol={}", symbol);
    let url = format!("https://api-testnet.bybit.com/v5/position/list?{}", params);
    let res = client
        .get(&url)
        .headers(construct_headers(&params))
        .send()
        .await?;
    let body = res.text().await?;

    let leverage_json: PositionList = serde_json::from_str(&body)?;

    let value: f32 = leverage_json.result.list[0].leverage.parse().unwrap();

    Ok(value)
}

async fn get_price(client: Client, symbol: &str) -> Result<f32, Box<dyn error::Error>> {
    let url = format!(
        "https://api-testnet.bybit.com/v5/market/tickers?category=linear&symbol={}",
        symbol
    );
    let res = client.get(&url).send().await?;
    let body = res.text().await?;

    let v: PriceInformation = serde_json::from_str(&body)?;

    let value: f32 = v.result.list[0].lastPrice.parse().unwrap();

    Ok(value)
}

async fn get_symbol_information(
    client: Client,
    symbol: &str,
) -> Result<f32, Box<dyn error::Error>> {
    let url = format!(
        "https://api-testnet.bybit.com/v5/market/instruments-info?category=linear&symbol={}",
        symbol
    );
    let res = client.get(&url).send().await?;
    let body = res.text().await?;

    let v: SymbolInformation = serde_json::from_str(&body)?;

    let value: f32 = v.result.list[0].lotSizeFilter.qtyStep.parse().unwrap();

    Ok(value)
}

async fn market_futures_position(
    client: Client,
    symbol: &str,
    side: &str,
    qty: f32,
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    let payload = format!(
        r#"{{"category":"linear","symbol":"{}","side":"{}","orderType":"Market","qty":"{}"}}"#,
        symbol, side, qty
    );

    info!("payload = {}", payload);

    let res = client
        .post(url)
        .headers(construct_headers(&payload))
        .body(payload)
        .send()
        .await?;
    let body = res.text().await?;

    let v: Value = serde_json::from_str(&body)?;
    info!("v = {}", v);

    Ok(())
}

enum TpCases {
    BinanceListing,
    UpbitListing,
    BinanceFuturesListing,
    NoListing,
}

fn title_case(title: &str) -> Result<(&str, TpCases), Box<dyn error::Error>> {
    if title.contains("Binance Will List") {
        Ok((r#"\([\d]*([^()]+)\)"#, TpCases::BinanceListing))
    } else if title.contains("마켓 디지털 자산 추가") {
        Ok((
            r#"\([\d]*([^()]+[^(\u3131-\u314e|\u314f-\u3163|\uac00-\ud7a3)])\)"#,
            TpCases::UpbitListing,
        ))
    } else if title.contains("Binance Futures Will Launch") {
        Ok((
            r#"(?<=USDⓈ-M )\d*(.*)(?= Perpetual)"#,
            TpCases::BinanceFuturesListing,
        ))
    } else {
        Ok(("", TpCases::NoListing))
    }
}

fn process_title(title: &str) -> Result<(&str, TpCases), Box<dyn error::Error>> {
    let (re_string, tp_case) = title_case(title)?;
    let re = Regex::new(re_string)?;
    let symbol = re
        .captures(title)
        .unwrap()
        .get(1)
        .map_or("", |m| m.as_str());

    Ok((symbol, tp_case))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Info)
        .init();
    let client = Client::new();
    let size: f32 = 200.0;
    if let Ok((mut socket, _)) = connect_async("ws://localhost:8765").await {
        let msg: Message = socket.next().await.expect("can't fetch data")?;

        let response = msg.to_text()?;

        let tree_response: TreeResponse = serde_json::from_str(&response)?;

        let (symbol, tp_case) = process_title(&tree_response.title)?;

        let trade_pair = format!("{}USDT", symbol);

        let qty_step: f32 = get_symbol_information(client.clone(), &trade_pair).await?;

        info!("qty_step = {}", qty_step);

        let leverage: f32 = get_leverage(client.clone(), &trade_pair).await?;

        info!("leverage = {}", leverage);

        let price: f32 = get_price(client.clone(), &trade_pair).await?;

        info!("price = {}", price);

        let qty = (size * leverage / price / qty_step).floor() * qty_step;

        info!("qty = {}", qty);

        market_futures_position(client.clone(), &trade_pair, "Buy", qty).await?;

        market_futures_position(client.clone(), &trade_pair, "Sell", qty).await?;
    } else {
        error!("Can't connect to test server");
    };

    Ok(())
}
