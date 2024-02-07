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
fn main() {
    println!("Hello, world!");
}
