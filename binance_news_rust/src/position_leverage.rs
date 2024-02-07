use serde::Deserialize;
//"[{\"symbol\":\"BTCUSDT\",\"positionAmt\":\"0.000\",\"entryPrice\":\"0.0\",\"breakEvenPrice\":\"0.0\",\"markPrice\":\"0.00000000\",\"unRealizedProfit\":\"0.00000000\",\"liquidationPrice\":\"0\",\"leverage\":\"1\",\"maxNotionalValue\":\"1.0E9\",\"marginType\":\"cross\",\"isolatedMargin\":\"0.00000000\",\"isAutoAddMargin\":\"false\",\"positionSide\":\"BOTH\",\"notional\":\"0\",\"isolatedWallet\":\"0\",\"updateTime\":0,\"isolated\":false,\"adlQuantile\":0}]"
#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PositionLeverage {
    pub leverage: String,
}
