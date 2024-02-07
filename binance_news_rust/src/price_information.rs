use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct PriceInformation {
    pub price: String,
}
