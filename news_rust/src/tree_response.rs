use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct TreeResponse {
    source: String,
    pub title: String,
}
