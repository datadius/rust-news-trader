mod order_information;
mod order_response;
mod position_list;
mod price_information;
mod symbol_information;
mod tree_response;

use order_information::OrderInformation;
use order_response::OrderResponse;
use position_list::PositionList;
use price_information::PriceInformation;
use symbol_information::SymbolInformation;
use tree_response::TreeResponse;

use env_logger;
use fraction::Decimal;
use futures::{future, StreamExt};
use hex;
use hmac::Mac;
use log::{error, info};
use regex::Regex;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use std::{collections::HashMap, env, error};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, tungstenite::Result};

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

async fn market_buy_futures_position(
    client: Client,
    symbol: &str,
    qty: f32,
    qty_step: f32,
    tp_instance_arr: &[TpInstance; 2],
    recv_window: &str,
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    let payload = format!(
        r#"{{"category":"linear","symbol":"{}","side":"Buy","orderType":"Market","qty":"{}"}}"#,
        symbol, qty
    );

    if let Ok(res) = client
        .post(url)
        .headers(construct_headers(&payload, recv_window))
        .body(payload)
        .send()
        .await
    {
        let body = res.text().await?;

        if tp_instance_arr[0].time != 0 {
            market_sell_position(
                client,
                symbol,
                qty,
                qty_step,
                "linear",
                tp_instance_arr,
                recv_window,
            )
            .await?;
        } else {
            error!("Failed to sell {}", body);
        }
    } else {
        error!("Error in sending the futures order");
    }
    Ok(())
}

async fn market_buy_spot_position(
    client: Client,
    symbol: &str,
    unit_qty: f32,
    qty_step: f32,
    tp_instance_arr: &[TpInstance; 2],
    recv_window: &str,
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    let payload = format!(
        r#"{{"category":"spot","symbol":"{}", "side":"Buy", "orderType":"Market","qty":"{}"}}"#,
        symbol, unit_qty
    );

    if let Ok(res) = client
        .post(url)
        .headers(construct_headers(&payload, recv_window))
        .body(payload)
        .send()
        .await
    {
        let body = res.text().await?;

        let order_response: Result<OrderResponse, _> = serde_json::from_str(&body);

        if let Ok(order) = order_response {
            if tp_instance_arr[0].time != 0 {
                let qty = get_order_qty(client.clone(), &order.result.orderId, recv_window).await?;
                let price = get_price(client.clone(), symbol).await?;

                let tp_qty = ((qty / price) / qty_step).floor() * qty_step;
                market_sell_position(
                    client,
                    symbol,
                    tp_qty,
                    qty_step,
                    "spot",
                    tp_instance_arr,
                    recv_window,
                )
                .await?;
            } else {
                error!("Failed to sell {}", body);
            }
        } else {
            error!("Failed to buy {}", body);
        }
    } else {
        error!("Error in sending the spot order");
    }

    Ok(())
}

async fn market_sell_position(
    client: Client,
    symbol: &str,
    qty: f32,
    qty_step: f32,
    category: &str,
    tp_instance_arr: &[TpInstance; 2],
    recv_window: &str,
) -> Result<(), Box<dyn error::Error>> {
    let url = "https://api-testnet.bybit.com/v5/order/create";

    for tp in tp_instance_arr {
        let seconds: u64 = tp.time as u64;
        sleep(Duration::from_secs(seconds)).await;
        let tp_pct = Decimal::from(tp.pct);
        let qty_step_dec = Decimal::from(qty_step);
        let qty_dec = Decimal::from(qty);
        let tp_qty = ((qty_dec / qty_step_dec) * tp_pct).floor() * qty_step_dec;
        let payload = format!(
            r#"{{"category":"{}","symbol":"{}","side":"Sell","orderType":"Market","qty":"{}"}}"#,
            category, symbol, tp_qty
        );

        info!("payload = {}", payload);

        let res = client
            .post(url)
            .headers(construct_headers(&payload, recv_window))
            .body(payload)
            .send()
            .await?;
        let body = res.text().await?;

        info!("Sell Status = {}, Category = {}", &body, category);
    }
    Ok(())
}

async fn get_leverage(
    client: Client,
    symbol: &str,
    recv_window: &str,
) -> Result<f32, Box<dyn error::Error>> {
    let params = format!("category=linear&symbol={}", symbol);
    let url = format!("https://api-testnet.bybit.com/v5/position/list?{}", params);
    let res = client
        .get(&url)
        .headers(construct_headers(&params, recv_window))
        .send()
        .await?;
    let body = res.text().await?;

    info!("Leverage = {}", body);

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

async fn get_order_qty(
    client: Client,
    order_id: &str,
    recv_window: &str,
) -> Result<f32, Box<dyn error::Error>> {
    let params = format!("category=spot&order_id={}", order_id);
    let url = format!("https://api-testnet.bybit.com/v5/order/history?{}", params);
    let res = client
        .get(&url)
        .headers(construct_headers(&params, recv_window))
        .send()
        .await?;
    let body = res.text().await?;

    let order_json: OrderInformation = serde_json::from_str(&body)?;

    let qty: f32 = order_json.result.list[0].qty.parse().unwrap();

    Ok(qty)
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
    if tp_case == TpCases::NoListing {
        return Ok(("", tp_case));
    }
    let re = Regex::new(re_string)?;
    let symbol = re
        .captures(title)
        .unwrap()
        .get(1)
        .map_or("", |m| m.as_str());

    Ok((symbol, tp_case))
}

fn construct_headers(payload: &str, recv_window: &str) -> HeaderMap {
    let api_key = env::var("testnet_bybit_order_key").expect("BYBIT_API_KEY not set");
    let api_secret = env::var("testnet_bybit_order_secret").expect("BYBIT_API_SECRET not set");
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn error::Error>> {
    let args: Vec<String> = env::args().collect();

    env_logger::Builder::new()
        .filter(None, log::LevelFilter::Info)
        .init();
    let client = Client::new();
    let size_future: f32 = args
        .get(1)
        .expect("Input size for futures")
        .parse()
        .unwrap_or(0.0);
    let size_spot: f32 = args
        .get(2)
        .unwrap_or(&String::from("0.0"))
        .parse()
        .unwrap_or(0.0);
    let default_recv_window = &String::from("1000");
    let recv_window: &str = args.get(3).unwrap_or(default_recv_window);
    let mut tp_map = HashMap::new();
    tp_map.insert(
        TpCases::BinanceListing,
        [
            //change the time to 2 * 60
            TpInstance {
                time: 30,
                pct: 0.75,
            },
            // 8 * 60
            TpInstance {
                time: 45,
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
    loop {
        //wss://news.treeofalpha.com/ws ws://35.73.200.147:5050
        if let Ok((mut socket, _)) = connect_async("wss://news.treeofalpha.com/ws").await {
            while let Some(msg) = socket.next().await {
                let msg = msg.unwrap_or(Message::binary(Vec::new()));

                if msg.is_text() {
                    let response = msg.to_text()?;

                    info!("Response = {}", response);

                    let tree_response: TreeResponse = serde_json::from_str(&response)?;

                    let (symbol, tp_case) = process_title(&tree_response.title)?;

                    if tp_case != TpCases::NoListing {
                        info!("symbol = {}", symbol);

                        let tp_instance_arr = tp_map.get(&tp_case).unwrap_or(&EMPTY_TP_CASE);

                        let trade_pair = format!("{}USDT", symbol);

                        let qty_step: f32 =
                            get_symbol_information(client.clone(), &trade_pair).await?;

                        let leverage: f32 =
                            get_leverage(client.clone(), &trade_pair, recv_window).await?;

                        info!("leverage = {}", leverage);

                        let price: f32 = get_price(client.clone(), &trade_pair).await?;

                        info!("price = {}", price);

                        let qty = (size_future * leverage / price / qty_step).floor() * qty_step;

                        info!("qty = {}", qty);

                        let (_s, _g) = future::join(
                            async {
                                market_buy_futures_position(
                                    client.clone(),
                                    &trade_pair,
                                    qty,
                                    qty_step,
                                    tp_instance_arr,
                                    recv_window,
                                )
                                .await
                            },
                            async {
                                market_buy_spot_position(
                                    client.clone(),
                                    &trade_pair,
                                    size_spot,
                                    qty_step,
                                    tp_instance_arr,
                                    recv_window,
                                )
                                .await
                            },
                        )
                        .await;
                    } else {
                        info!("Not a listing {}", &tree_response.title)
                    }
                }
            }
        } else {
            error!("Can't connect to test server");
        };
    }
}
