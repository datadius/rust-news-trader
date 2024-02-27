use super::generate_headers_and_signature;
use super::get_price;
use super::get_trade_pair_leverage;
use super::process_title;
use super::update_symbol_information;
use super::TpCases;
use fancy_regex::Regex;
use fraction::Decimal;
use futures::{future, StreamExt};
use hex;
use hmac::Mac;
use log::{error, info};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};

use std::{collections::HashMap, env, error};
use tokio::time::{sleep, Duration};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message, tungstenite::Result};

#[test]
fn test_process_title_variants() {
    let title_binance_listing = "Binance Will List Dymension (DYM) with Seed Tag Applied";
    let (symbol, tp_case) =
        process_title(title_binance_listing).expect("Error processing binance listing");

    assert_eq!(vec!["DYM"], symbol);
    assert_eq!(TpCases::BinanceListing, tp_case);

    let title_upbit_listing = "KRW 마켓 디지털 자산 추가 (CTC)";
    let (symbol, tp_case) =
        process_title(title_upbit_listing).expect("Error processing upbit listing");

    assert_eq!(vec!["CTC"], symbol);
    assert_eq!(TpCases::UpbitListing, tp_case);

    let title_binance_futures_listing =
        "Binance Futures Will Launch USDⓈ-M ZETA Perpetual Contract With Up to 50x Leverage";
    let (symbol, tp_case) = process_title(title_binance_futures_listing)
        .expect("Error processing binance futures listing");

    assert_eq!(vec!["ZETA"], symbol);
    assert_eq!(TpCases::BinanceFuturesListing, tp_case);

    let title_binance_futures_1000sats =
        "Binance Futures Will Launch USDⓈ-M 1000SATS Perpetual Contract With Up to 50x Leverage";
    let (symbol, tp_case) = process_title(title_binance_futures_1000sats)
        .expect("Error processing binance futures listing");

    assert_eq!(vec!["SATS"], symbol);
    assert_eq!(TpCases::BinanceFuturesListing, tp_case);

    let title_empty = "";
    let (symbol, tp_case) = process_title(title_empty).expect("Error processing empty title");

    assert_eq!(vec![""], symbol);
    assert_eq!(TpCases::NoListing, tp_case);

    let title_random_text = "This is a random text";
    let (symbol, tp_case) = process_title(title_random_text).expect("Error processing random text");

    assert_eq!(vec![""], symbol);
    assert_eq!(TpCases::NoListing, tp_case);

    let title_bithumb_text = "맨틀(MNT) 원화 마켓 추가";
    let (symbol, tp_case) =
        process_title(title_bithumb_text).expect("Error processing bithumb text");

    assert_eq!(vec!["MNT"], symbol);
    assert_eq!(TpCases::BithumbListing, tp_case);

    let multiple_upbit_listing = "KRW, BTC 마켓 디지털 자산 추가 (ALT, PYTH)";
    let (symbol, tp_case) =
        process_title(multiple_upbit_listing).expect("Error processing multiple upbit listing");

    assert_eq!(vec!["ALT", "PYTH"], symbol);
    assert_eq!(TpCases::UpbitListing, tp_case);
}

#[test]
fn test_generate_headers_and_signature() {
    let current_timestamp = chrono::Utc::now().timestamp_millis().to_string();
    let api_key_spot = env::var("test_spot_binance_order_key").expect("Binance_API_KEY not set");
    let api_secret_spot =
        env::var("test_spot_binance_order_secret").expect("Binance_API_SECRET not set");
    let mut headers_spot = HeaderMap::new();
    headers_spot.insert(
        "X-MBX-APIKEY",
        HeaderValue::from_str(&api_key_spot).expect("Issue processing api key"),
    );
    let category_spot = "spot";

    let api_key_futures = env::var("testnet_binance_order_key").expect("Binance_API_KEY not set");
    let api_secret_futures =
        env::var("testnet_binance_order_secret").expect("Binance_API_SECRET not set");
    let mut headers_futures = HeaderMap::new();
    headers_futures.insert(
        "X-MBX-APIKEY",
        HeaderValue::from_str(&api_key_futures).expect("Issue processing api key"),
    );
    let payload_futures = "";
    let category_futures = "futures";

    let api_key_other = env::var("testnet_binance_order_key").expect("Binance_API_KEY not set");
    let api_secret_other =
        env::var("testnet_binance_order_secret").expect("Binance_API_SECRET not set");
    let mut headers_other = HeaderMap::new();
    headers_other.insert(
        "X-MBX-APIKEY",
        HeaderValue::from_str(&api_key_other).expect("Issue processing api key"),
    );
    let payload_other = "";
    let category_other = "";

    let payload_btcusdt = &format!(
        "symbol=BTCUSDT&recvWindow=5000&timestamp={}",
        &current_timestamp
    );
    let payload_empty = "";

    let payload_list: Vec<&str> = vec![payload_btcusdt, payload_empty];

    for payload in payload_list {
        let signature_spot = {
            type HmacSha256 = hmac::Hmac<sha2::Sha256>;
            let mut mac = HmacSha256::new_from_slice(&api_secret_spot.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(payload.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        let signature_futures = {
            type HmacSha256 = hmac::Hmac<sha2::Sha256>;
            let mut mac = HmacSha256::new_from_slice(&api_secret_futures.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(payload.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };
        let signature_other = {
            type HmacSha256 = hmac::Hmac<sha2::Sha256>;
            let mut mac = HmacSha256::new_from_slice(&api_secret_other.as_bytes())
                .expect("HMAC can take key of any size");
            mac.update(payload.as_bytes());
            hex::encode(mac.finalize().into_bytes())
        };

        let (headers, signature) = generate_headers_and_signature(category_spot, payload);
        assert_eq!(headers_spot, headers);
        assert_eq!(signature_spot, signature);

        let (headers, signature) = generate_headers_and_signature(category_futures, payload);
        assert_eq!(headers_futures, headers);
        assert_eq!(signature_futures, signature);

        let (headers, signature) = generate_headers_and_signature(category_other, payload);
        assert_eq!(headers_other, headers);
        assert_eq!(signature_other, signature);
    }
}

#[tokio::test]
async fn test_symbol_hashmap() -> Result<(), Box<dyn error::Error>> {
    let client = Client::new();
    let mut symbols_step_size: HashMap<String, f32> = HashMap::new();
    update_symbol_information(client.clone(), &mut symbols_step_size).await?;

    let mut trade_pair_assert_hashmap: HashMap<String, f32> = HashMap::new();

    trade_pair_assert_hashmap.insert("BTCUSDT".to_string(), 0.001);
    trade_pair_assert_hashmap.insert("".to_string(), 0.0);
    trade_pair_assert_hashmap.insert("PYTHUSDT".to_string(), 1.0);
    trade_pair_assert_hashmap.insert("ETCUSDT".to_string(), 0.01);
    trade_pair_assert_hashmap.insert("TWTUSDT".to_string(), 0.1);

    for (trade_pair, qty_step_assert) in trade_pair_assert_hashmap {
        let qty_step: f32 = symbols_step_size
            .get(&trade_pair)
            .unwrap_or(&0.0)
            .to_owned();
        assert_eq!(qty_step_assert, qty_step);
    }

    Ok(())
}

#[tokio::test]
async fn test_get_price() -> Result<(), Box<dyn error::Error>> {
    let client = Client::new();
    let trade_pair = "BTCUSDT";

    let price = get_price(client.clone(), trade_pair).await?;

    assert_ne!(price, 0.0);

    let trade_pair_empty = "";

    let price_empty = get_price(client.clone(), trade_pair_empty).await?;

    assert_eq!(price_empty, 0.0);

    let trade_pair_invalid = "INVALID";

    let price_invalid = get_price(client.clone(), trade_pair_invalid).await?;

    assert_eq!(price_invalid, 0.0);

    Ok(())
}

#[tokio::test]
async fn test_get_leverage() -> Result<(), Box<dyn error::Error>> {
    let client = Client::new();
    let trade_pair = "BTCUSDT";
    let recv_window = "5000";

    let leverage = get_trade_pair_leverage(client.clone(), trade_pair, recv_window).await?;

    assert_ne!(leverage, 0.0);

    let trade_pair_empty = "";

    let leverage_empty =
        get_trade_pair_leverage(client.clone(), trade_pair_empty, recv_window).await?;

    assert_eq!(leverage_empty, 20.0);

    let trade_pair_invalid = "INVALID";

    let leverage_invalid =
        get_trade_pair_leverage(client.clone(), trade_pair_invalid, recv_window).await?;

    assert_eq!(leverage_invalid, 0.0);

    Ok(())
}
