use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct OrderResponse {
    pub result: OrderResponseResult,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct OrderResponseResult {
    pub orderId: String,
}
