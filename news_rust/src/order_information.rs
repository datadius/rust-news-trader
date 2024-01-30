use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct OrderInformation {
    pub result: OrderInformationResult,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct OrderInformationResult {
    pub list: Vec<OrderInformationList>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct OrderInformationList {
    pub cumExecQty: String,
    pub cumExecFee: String,
}
