use super::process_title;
use super::TpCases;

#[test]
fn test_process_title_variants() {
    let title_binance_listing = "Binance Will List Dymension (DYM) with Seed Tag Applied";
    let (symbol, tp_case) = process_title(title_binance_listing).expect("Error processing binance listing");

    assert_eq!("DYM",symbol);
    assert_eq!(TpCases::BinanceListing, tp_case);


    let title_upbit_listing = "KRW 마켓 디지털 자산 추가 (CTC)";
    let (symbol, tp_case) = process_title(title_upbit_listing).expect("Error processing upbit listing");

    assert_eq!("CTC",symbol);
    assert_eq!(TpCases::UpbitListing, tp_case);

    let title_binance_futures_listing = "Binance Futures Will Launch USDⓈ-M ZETA Perpetual Contract With Up to 50x Leverage";
    let (symbol, tp_case) = process_title(title_binance_futures_listing).expect("Error processing binance futures listing");

    assert_eq!("ZETA",symbol);
    assert_eq!(TpCases::BinanceFuturesListing, tp_case);

    let title_binance_futures_1000sats = "Binance Futures Will Launch USDⓈ-M 1000SATS Perpetual Contract With Up to 50x Leverage";
    let (symbol, tp_case) = process_title(title_binance_futures_1000sats).expect("Error processing binance futures listing");

    assert_eq!("SATS",symbol);
    assert_eq!(TpCases::BinanceFuturesListing, tp_case);

    let title_empty = "";
    let (symbol, tp_case) = process_title(title_empty).expect("Error processing empty title");

    assert_eq!("",symbol);
    assert_eq!(TpCases::NoListing, tp_case);


    let title_random_text = "This is a random text";
    let (symbol, tp_case) = process_title(title_random_text).expect("Error processing random text");

    assert_eq!("",symbol);
    assert_eq!(TpCases::NoListing, tp_case);

    let title_bithumb_text = "맨틀(MNT) 원화 마켓 추가";
    let (symbol, tp_case) = process_title(title_bithumb_text).expect("Error processing bithumb text");

    assert_eq!("MNT",symbol);
    assert_eq!(TpCases::BithumbListing, tp_case);

}
