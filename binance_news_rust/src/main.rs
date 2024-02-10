mod position_leverage;
mod price_information;
mod symbols_exchange_info;
mod tree_response;

use position_leverage::PositionLeverage;
use price_information::PriceInformation;
use symbols_exchange_info::ExchangeInfo;
use tree_response::TreeResponse;

use env_logger;
use fraction::{Decimal, Fraction};
use futures::{executor, future, SinkExt, StreamExt};
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
fn generate_headers_and_signature(category: &str, payload: &str) -> (HeaderMap, String) {
    let to_sign = payload;
    let api_key = match category {
        "spot" => env::var("test_spot_binance_order_key").expect("Binance_API_KEY not set"),
        "futures" => env::var("testnet_binance_order_key").expect("Binance_API_KEY not set"),
        _ => env::var("testnet_binance_order_key").expect("Binance_API_KEY not set"),
    };
    let api_secret = match category {
        "spot" => env::var("test_spot_binance_order_secret").expect("Binance_API_SECRET not set"),
        "futures" => env::var("testnet_binance_order_secret").expect("Binance_API_SECRET not set"),
        _ => env::var("testnet_binance_order_secret").expect("Binance_API_SECRET not set"),
    };

    let signature = {
        type HmacSha256 = hmac::Hmac<sha2::Sha256>;
        let mut mac = HmacSha256::new_from_slice(&api_secret.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(to_sign.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    };

    let mut headers = HeaderMap::new();
    headers.insert("X-MBX-APIKEY", HeaderValue::from_str(&api_key).unwrap());
    (headers, signature)
}

async fn update_symbol_information(
    client: Client,
    symbols_step_size: &mut HashMap<String, f32>,
) -> Result<(), Box<dyn error::Error>> {
    if let Ok(response) = client
        .get("https://testnet.binancefuture.com/fapi/v1/exchangeInfo")
        .send()
        .await
    {
        let body = response.text().await?;
        let exchange_info: ExchangeInfo = serde_json::from_str(&body)?;
        for symbol in exchange_info.symbols {
            let quantity_precision = symbol.quantityPrecision;
            let step_size = 10_f32.powf(-quantity_precision as f32);
            symbols_step_size.insert(symbol.symbol.to_owned(), step_size);
        }
    }
    Ok(())
}

async fn get_price(client: Client, symbol: &str) -> Result<f32, Box<dyn error::Error>> {
    if let Ok(response) = client
        .get("https://testnet.binancefuture.com/fapi/v1/ticker/price")
        .query(&[("symbol", symbol)])
        .send()
        .await
    {
        let body = response.text().await?;
        let price_information: PriceInformation = serde_json::from_str(&body)?;
        let price = price_information.price.parse::<f32>().unwrap();
        Ok(price)
    } else {
        error!("Failed to get price for {}", symbol);
        Ok(0.0)
    }
}

async fn get_trade_pair_leverage(
    client: Client,
    symbol: &str,
    recv_window: &str,
) -> Result<f32, Box<dyn error::Error>> {
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = format!(
        "symbol={}&recvWindow={}&timestamp={}",
        symbol, recv_window, &current_timestamp
    );
    let (headers, signature) = generate_headers_and_signature("futures", &payload);
    if let Ok(response) = client
        .get("https://testnet.binancefuture.com/fapi/v2/positionRisk")
        .query(&[
            ("symbol", symbol),
            ("recvWindow", recv_window),
            ("timestamp", &current_timestamp),
            ("signature", &signature),
        ])
        .headers(headers)
        .send()
        .await
    {
        let body = response.text().await?;
        let position_risk: Vec<PositionLeverage> = serde_json::from_str(&body)?;
        let leverage = position_risk[0].leverage.parse::<f32>().unwrap_or(0.0);
        Ok(leverage)
    } else {
        error!("Failed to get leverage for {}", symbol);
        Ok(0.0)
    }
}

async fn market_buy_futures_position(
    client: Client,
    symbol: &str,
    base_coin_qty: f32,
    qty_step: f32,
    tp_instance_arr: &[TpInstance; 2],
    recv_window: &str,
) -> Result<(), Box<dyn error::Error>> {
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = format!(
        "symbol={}&side=BUY&type=MARKET&quantity={}&recvWindow={}&timestamp={}",
        symbol, base_coin_qty, recv_window, &current_timestamp
    );
    let (headers, signature) = generate_headers_and_signature("futures", &payload);
    if let Ok(response) = client
        .post("https://testnet.binancefuture.com/fapi/v1/order")
        .query(&[
            ("symbol", symbol),
            ("side", "BUY"),
            ("type", "MARKET"),
            ("quantity", &base_coin_qty.to_string()),
            ("recvWindow", recv_window),
            ("timestamp", &current_timestamp),
            ("signature", &signature),
        ])
        .headers(headers)
        .send()
        .await
    {
        let body = response.text().await?;
        info!("Market buy futures position response: {}", body);
        market_sell_position(
            client,
            symbol,
            base_coin_qty,
            qty_step,
            "futures",
            tp_instance_arr,
            recv_window,
        )
        .await?;
        Ok(())
    } else {
        error!("Failed to market buy futures position for {}", symbol);
        Ok(())
    }
}

async fn market_buy_spot_position(
    client: Client,
    symbol: &str,
    unit_coin_qty: f32,
    tp_instance_arr: &[TpInstance; 2],
    recv_window: &str,
) -> Result<(), Box<dyn error::Error>> {
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let payload = format!(
        "symbol={}&side=BUY&type=MARKET&quoteOrderQty={}&recvWindow={}&timestamp={}",
        symbol, unit_coin_qty, recv_window, &current_timestamp
    );
    let (headers, signature) = generate_headers_and_signature("spot", &payload);
    if let Ok(response) = client
        .post("https://testnet.binance.vision/api/v3/order")
        .query(&[
            ("symbol", symbol),
            ("side", "BUY"),
            ("type", "MARKET"),
            ("quoteOrderQty", &unit_coin_qty.to_string()),
            ("recvWindow", recv_window),
            ("timestamp", &current_timestamp),
            ("signature", &signature),
        ])
        .headers(headers)
        .send()
        .await
    {
        let body = response.text().await?;
        info!("Market buy spot position response: {}", body);
        market_sell_position(
            client,
            symbol,
            unit_coin_qty,
            0.0,
            "spot",
            tp_instance_arr,
            recv_window,
        )
        .await?;
        Ok(())
    } else {
        error!("Failed to market buy spot position for {}", symbol);
        Ok(())
    }
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
    let url = match category {
        "futures" => "https://testnet.binancefuture.com/fapi/v1/order",
        "spot" => "https://testnet.binance.vision/api/v3/order",
        _ => "",
    };

    for tp in tp_instance_arr {
        let seconds: u64 = tp.time as u64;
        sleep(Duration::from_secs(seconds)).await;
        let tp_pct = Decimal::from(tp.pct);
        let qty_step_dec = Decimal::from(qty_step);
        let qty_dec = Decimal::from(qty);
        let tp_qty = match category {
            "futures" => ((qty_dec / qty_step_dec) * tp_pct).floor() * qty_step_dec,
            "spot" => qty_dec * tp_pct,
            _ => Decimal::from(0),
        };
        let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
        let payload = match category {
            "futures" => format!(
                "symbol={}&side=SELL&type=MARKET&quantity={}&recvWindow={}&timestamp={}",
                symbol, tp_qty, recv_window, &current_timestamp
            ),
            "spot" => format!(
                "symbol={}&side=SELL&type=MARKET&quoteOrderQty={}&recvWindow={}&timestamp={}",
                symbol, tp_qty, recv_window, &current_timestamp
            ),
            _ => "".to_string(),
        };

        let (headers, signature) = generate_headers_and_signature(category, &payload);

        if let Ok(response) = client
            .post(url)
            .query(&[
                ("symbol", symbol),
                ("side", "SELL"),
                ("type", "MARKET"),
                ("quoteOrderQty", &tp_qty.to_string()),
                ("recvWindow", recv_window),
                ("timestamp", &current_timestamp),
                ("signature", &signature),
            ])
            .headers(headers)
            .send()
            .await
        {
            let body = response.text().await?;
            info!("Market sell position response: {}", body);
        } else {
            error!("Failed to market sell position for {}", symbol);
        }
    }

    Ok(())
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

    let mut symbols_step_size: HashMap<String, f32> = HashMap::new();
    update_symbol_information(client.clone(), &mut symbols_step_size).await?;
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

                        let trade_pair = format!("{}USDT", symbol);

                        let tp_instance_arr = tp_map.get(&tp_case).unwrap_or(&EMPTY_TP_CASE);

                        let qty_step: f32 = symbols_step_size
                            .get(&trade_pair)
                            .unwrap_or(&0.0)
                            .to_owned();
                        let price: f32 = get_price(client.clone(), &trade_pair).await?;
                        let leverage: f32 =
                            get_trade_pair_leverage(client.clone(), &trade_pair, recv_window)
                                .await?;

                        let base_coin_qty =
                            (size_future * leverage / price / qty_step).floor() * qty_step;
                        info!("Base coin qty: {}", base_coin_qty);

                        let (_s, _g) = future::join(
                            async {
                                market_buy_futures_position(
                                    client.clone(),
                                    &trade_pair,
                                    base_coin_qty,
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
                                    tp_instance_arr,
                                    recv_window,
                                )
                                .await
                            },
                        )
                        .await;
                    } else {
                        info!("No listing for {}", symbol);
                    }
                    update_symbol_information(client.clone(), &mut symbols_step_size).await?;
                }
            }
        } else {
            error!("Failed to connect to the server");
        };
    }
}
