use serde::Deserialize;

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct SymbolInformation {
    pub result: ListSymbols,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct ListSymbols {
    pub list: Vec<Symbol>,
}
#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct Symbol {
    pub lotSizeFilter: LotSizeFilter,
}
#[allow(non_snake_case)]
#[derive(Deserialize)]
pub struct LotSizeFilter {
    pub qtyStep: String,
}
