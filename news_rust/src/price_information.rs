use serde::Deserialize;
//{"result":{"category":"linear","list":[{"ask1Price":"43328.00","ask1Size":"0.035","basis":"","basisRate":"","bid1Price":"43327.90","bid1Size":"11.341","deliveryFeeRate":"","deliveryTime":"0","fundingRate":"0.000042","highPrice24h":"43890.80","indexPrice":"43352.28","lastPrice":"43327.40","lowPrice24h":"42954.70","markPrice":"43327.77","nextFundingTime":"1706659200000","openInterest":"59315.727","openInterestValue":"2570018176.84","predictedDeliveryPrice":"","prevPrice1h":"43412.80","prevPrice24h":"43042.20","price24hPcnt":"0.006626","symbol":"BTCUSDT","turnover24h":"4908164053.9815","volume24h":"113140.3870"}]},"retCode":0,"retExtInfo":{},"retMsg":"OK","time":1706641454555}
//
#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PriceInformation {
    pub result: PriceInformationResult,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PriceInformationResult {
    pub list: Vec<PriceInformationList>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PriceInformationList {
    pub lastPrice: String,
}
