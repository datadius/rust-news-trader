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
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use serde_json::Value;
use std::{collections::HashMap, env, error, io::Read};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{
    connect_async,
    tungstenite::protocol::Message,
    tungstenite::{Error, Result},
};

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

    //Remember to replace this with qty because standard account doesn't have these values
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

    let price_information: PriceInformation = serde_json::from_str(&body)?;

    let value: f32 = price_information.result.list[0].lastPrice.parse().unwrap();

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

    let symbol_information: SymbolInformation = serde_json::from_str(&body)?;

    let value: f32 = symbol_information.result.list[0]
        .lotSizeFilter
        .qtyStep
        .parse()
        .unwrap();

    Ok(value)
}

async fn market_buy_futures_position(
    client: Client,
    symbol: &str,
    qty: f32,
    qty_step: f32,
    tp_instance_arr: &[TpInstance; 2],
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    let payload = format!(
        r#"{{"category":"linear","symbol":"{}","side":"Buy","orderType":"Market","qty":"{}"}}"#,
        symbol, qty
    );

    info!("payload = {}", payload);

    if let Ok(res) = client
        .post(url)
        .headers(construct_headers(&payload))
        .body(payload)
        .send()
        .await
    {
        let body = res.text().await?;

        let v: Value = serde_json::from_str(&body)?;
        info!("v = {}", v);
        if tp_instance_arr[0].time != 0 {
            market_sell_futures_position(client, symbol, qty, qty_step, tp_instance_arr).await?;
        } else {
            info!("Failed to sell {}", v);
        }
    } else {
        error!("Error in market_buy_futures_position");
    }
    Ok(())
}

async fn market_sell_futures_position(
    client: Client,
    symbol: &str,
    qty: f32,
    qty_step: f32,
    tp_instance_arr: &[TpInstance; 2],
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    for tp in tp_instance_arr {
        let seconds: u64 = tp.time as u64;
        sleep(Duration::from_secs(seconds)).await;
        let tp_pct = &tp.pct;
        let tp_qty = ((qty / qty_step) * tp_pct).floor() * qty_step;
        let payload = format!(
            r#"{{"category":"linear","symbol":"{}","side":"Sell","orderType":"Market","qty":"{}"}}"#,
            symbol, tp_qty
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
    }
    Ok(())
}

#[derive(Eq, PartialEq, Hash)]
enum TpCases {
    BinanceListing,
    UpbitListing,
    BinanceFuturesListing,
    NoListing,
}

#[derive(Copy, Clone)]
struct TpInstance {
    time: u64,
    pct: f32,
}
const EMPTY_TP_CASE: [TpInstance; 2] = [TpInstance { time: 0, pct: 0.0 }; 2];

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
    let mut tp_map = HashMap::new();
    tp_map.insert(
        TpCases::BinanceListing,
        [
            TpInstance {
                time: 2 * 60,
                pct: 0.75,
            },
            TpInstance {
                time: 8 * 60,
                pct: 0.25,
            },
        ],
    );
    tp_map.insert(
        TpCases::UpbitListing,
        [
            TpInstance {
                time: 2 * 60,
                pct: 0.75,
            },
            TpInstance {
                time: 13 * 60,
                pct: 0.25,
            },
        ],
    );
    tp_map.insert(
        TpCases::BinanceFuturesListing,
        [
            TpInstance { time: 7, pct: 0.5 },
            TpInstance {
                time: 2 * 60,
                pct: 0.5,
            },
        ],
    );
    if let Ok((mut socket, _)) = connect_async("ws://localhost:8765").await {
        let msg: Message = socket.next().await.expect("can't fetch data")?;

        let response = msg.to_text()?;

        let tree_response: TreeResponse = serde_json::from_str(&response)?;

        let (symbol, tp_case) = process_title(&tree_response.title)?;

        if tp_case != TpCases::NoListing {
            let tp_instance_arr = tp_map.get(&tp_case).unwrap_or(&EMPTY_TP_CASE);

            let trade_pair = format!("{}USDT", symbol);

            let qty_step: f32 = get_symbol_information(client.clone(), &trade_pair).await?;

            let leverage: f32 = get_leverage(client.clone(), &trade_pair).await?;

            info!("leverage = {}", leverage);

            let price: f32 = get_price(client.clone(), &trade_pair).await?;

            info!("price = {}", price);

            let qty = (size * leverage / price / qty_step).floor() * qty_step;

            info!("qty = {}", qty);

            market_buy_futures_position(
                client.clone(),
                &trade_pair,
                qty,
                qty_step,
                tp_instance_arr,
            )
            .await?;
        } else {
            info!("{}", &tree_response.title)
        }
    } else {
        error!("Can't connect to test server");
    };

    Ok(())
}
