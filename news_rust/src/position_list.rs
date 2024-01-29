use serde::Deserialize;
//{"result":{"category":"linear","list":[{"adlRankIndicator":0,"autoAddMargin":0,"avgPrice":"0","bustPrice":"","createdTime":"0","cumRealisedPnl":"-1.57895924","isReduceOnly":false,"leverage":"1","leverageSysUpdatedTime":"","liqPrice":"","markPrice":"43054.8","mmrSysUpdatedTime":"","positionBalance":"0","positionIM":"0","positionIdx":0,"positionMM":"0","positionStatus":"Normal","positionValue":"","riskId":1,"riskLimitValue":"2000000","seq":119030911540,"side":"","size":"0","stopLoss":"","symbol":"BTCUSDT","takeProfit":"","tpslMode":"Full","tradeMode":0,"trailingStop":"0","unrealisedPnl":"","updatedTime":"1705499116177"}],"nextPageCursor":"BTCUSDT%2C1705499116177%2C0"},"retCode":0,"retExtInfo":{},"retMsg":"OK","time":1706557732801}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PositionList {
    pub result: Result,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct Result {
    pub list: Vec<LeverageList>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct LeverageList {
    pub leverage: String,
}
