use crate::domain::{events::*, types::*};
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::str::FromStr;

/// Raw JSON structures for parsing the provided data files
/// Raw order book snapshot from JSON
#[derive(Debug, Deserialize)]
struct RawOrderBookSnapshot {
    #[serde(rename = "type")]
    event_type: String,
    symbol: String,
    seq: u64,
    ts: String,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

/// Raw order book delta from JSON
#[derive(Debug, Deserialize)]
struct RawOrderBookDelta {
    #[serde(rename = "type")]
    event_type: String,
    symbol: String,
    seq: u64,
    ts: String,
    bids: Vec<Vec<String>>,
    asks: Vec<Vec<String>>,
}

/// Raw AMM pool from JSON
#[derive(Debug, Deserialize)]
struct RawAMMPool {
    pool: String,
    fee: u32,
    ts: String,
    reserves: RawReserves,
    #[serde(rename = "sqrtPriceX96")]
    sqrt_price_x96: Option<String>,
}

/// Raw reserves from JSON
#[derive(Debug, Deserialize)]
struct RawReserves {
    amount0: String,
    amount1: String,
}

/// Parse order book snapshot from JSON string
pub fn parse_lob_snapshot(json_str: &str) -> TradingResult<(Symbol, OrderBookSnapshot)> {
    let raw: RawOrderBookSnapshot = serde_json::from_str(json_str)?;

    if raw.event_type != "snapshot" {
        return Err(TradingError::ParseError(format!(
            "Expected snapshot type, got: {}",
            raw.event_type
        )));
    }

    let symbol = Symbol(raw.symbol);
    let sequence = SequenceNumber(raw.seq);
    let timestamp = parse_timestamp(&raw.ts)?;

    let mut bids = BTreeMap::new();
    let mut asks = BTreeMap::new();

    // Parse bids (price, quantity pairs)
    for bid in raw.bids {
        if bid.len() != 2 {
            return Err(TradingError::ParseError("Invalid bid format".to_string()));
        }
        let price = Price(
            Decimal::from_str(&bid[0])
                .map_err(|e| TradingError::ParseError(format!("Invalid bid price: {}", e)))?,
        );
        let quantity = Quantity(
            Decimal::from_str(&bid[1])
                .map_err(|e| TradingError::ParseError(format!("Invalid bid quantity: {}", e)))?,
        );
        bids.insert(price, quantity);
    }

    // Parse asks (price, quantity pairs)
    for ask in raw.asks {
        if ask.len() != 2 {
            return Err(TradingError::ParseError("Invalid ask format".to_string()));
        }
        let price = Price(
            Decimal::from_str(&ask[0])
                .map_err(|e| TradingError::ParseError(format!("Invalid ask price: {}", e)))?,
        );
        let quantity = Quantity(
            Decimal::from_str(&ask[1])
                .map_err(|e| TradingError::ParseError(format!("Invalid ask quantity: {}", e)))?,
        );
        asks.insert(price, quantity);
    }

    let snapshot = OrderBookSnapshot::with_levels(sequence, timestamp, bids, asks);
    Ok((symbol, snapshot))
}

/// Parse order book delta from JSON string
pub fn parse_lob_delta(json_str: &str) -> TradingResult<(Symbol, OrderBookDelta)> {
    let raw: RawOrderBookDelta = serde_json::from_str(json_str)?;

    if raw.event_type != "delta" {
        return Err(TradingError::ParseError(format!(
            "Expected delta type, got: {}",
            raw.event_type
        )));
    }

    let symbol = Symbol(raw.symbol);
    let sequence = SequenceNumber(raw.seq);
    let timestamp = parse_timestamp(&raw.ts)?;

    let mut updates = Vec::new();

    // Parse bid updates (action, price, quantity)
    for bid in raw.bids {
        if bid.len() != 3 {
            return Err(TradingError::ParseError(
                "Invalid bid delta format".to_string(),
            ));
        }

        let action = match bid[0].as_str() {
            "u" => UpdateAction::Update,
            "d" => UpdateAction::Delete,
            "n" => UpdateAction::Update, // New level is same as update
            _ => {
                return Err(TradingError::ParseError(format!(
                    "Invalid bid action: {}",
                    bid[0]
                )))
            }
        };

        let price = Price(
            Decimal::from_str(&bid[1])
                .map_err(|e| TradingError::ParseError(format!("Invalid bid price: {}", e)))?,
        );

        let quantity =
            if action == UpdateAction::Delete {
                Quantity::zero()
            } else {
                Quantity(Decimal::from_str(&bid[2]).map_err(|e| {
                    TradingError::ParseError(format!("Invalid bid quantity: {}", e))
                })?)
            };

        updates.push(PriceLevelUpdate::new(Side::Bid, price, quantity, action));
    }

    // Parse ask updates (action, price, quantity)
    for ask in raw.asks {
        if ask.len() != 3 {
            return Err(TradingError::ParseError(
                "Invalid ask delta format".to_string(),
            ));
        }

        let action = match ask[0].as_str() {
            "u" => UpdateAction::Update,
            "d" => UpdateAction::Delete,
            "n" => UpdateAction::Update, // New level is same as update
            _ => {
                return Err(TradingError::ParseError(format!(
                    "Invalid ask action: {}",
                    ask[0]
                )))
            }
        };

        let price = Price(
            Decimal::from_str(&ask[1])
                .map_err(|e| TradingError::ParseError(format!("Invalid ask price: {}", e)))?,
        );

        let quantity =
            if action == UpdateAction::Delete {
                Quantity::zero()
            } else {
                Quantity(Decimal::from_str(&ask[2]).map_err(|e| {
                    TradingError::ParseError(format!("Invalid ask quantity: {}", e))
                })?)
            };

        updates.push(PriceLevelUpdate::new(Side::Ask, price, quantity, action));
    }

    let delta = OrderBookDelta::with_updates(sequence, timestamp, updates);
    Ok((symbol, delta))
}

/// Parse AMM pool data from JSON string
pub fn parse_amm_pool(json_str: &str) -> TradingResult<(PoolAddress, AMMPoolUpdate)> {
    let raw: RawAMMPool = serde_json::from_str(json_str)?;

    let address = PoolAddress(raw.pool);
    let timestamp = parse_timestamp(&raw.ts)?;
    let fee_tier = FeeTier(raw.fee);

    let token0_reserve = Decimal::from_str(&raw.reserves.amount0)
        .map_err(|e| TradingError::ParseError(format!("Invalid token0 reserve: {}", e)))?;
    let token1_reserve = Decimal::from_str(&raw.reserves.amount1)
        .map_err(|e| TradingError::ParseError(format!("Invalid token1 reserve: {}", e)))?;
    let reserves = TokenReserves::new(token0_reserve, token1_reserve);

    // Parse sqrt price if available
    let sqrt_price = if let Some(sqrt_price_str) = raw.sqrt_price_x96 {
        let sqrt_price_u128 = u128::from_str(&sqrt_price_str)
            .map_err(|e| TradingError::ParseError(format!("Invalid sqrtPriceX96: {}", e)))?;
        Some(SqrtPriceX96(sqrt_price_u128))
    } else {
        None
    };

    let update = AMMPoolUpdate::new(timestamp, reserves, sqrt_price, fee_tier);
    Ok((address, update))
}

/// Parse timestamp from ISO 8601 string
fn parse_timestamp(ts_str: &str) -> TradingResult<Timestamp> {
    chrono::DateTime::parse_from_rfc3339(ts_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .map_err(|e| TradingError::ParseError(format!("Invalid timestamp: {}", e)))
}

/// Load and parse order book snapshot from file
pub fn load_lob_snapshot(file_path: &str) -> TradingResult<(Symbol, OrderBookSnapshot)> {
    let content = std::fs::read_to_string(file_path)?;
    parse_lob_snapshot(&content)
}

/// Load and parse order book delta from file
pub fn load_lob_delta(file_path: &str) -> TradingResult<(Symbol, OrderBookDelta)> {
    let content = std::fs::read_to_string(file_path)?;
    parse_lob_delta(&content)
}

/// Load and parse AMM pool from file
pub fn load_amm_pool(file_path: &str) -> TradingResult<(PoolAddress, AMMPoolUpdate)> {
    let content = std::fs::read_to_string(file_path)?;
    parse_amm_pool(&content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn test_parse_lob_snapshot() {
        let json = r#"{
            "type": "snapshot",
            "venue": "binance",
            "symbol": "ETHUSDC",
            "seq": 1000,
            "ts": "2025-01-01T10:00:00.000Z",
            "bids": [
                ["2445.12", "3.1005"],
                ["2445.11", "1.20"]
            ],
            "asks": [
                ["2445.13", "2.05"],
                ["2445.14", "1.50"]
            ]
        }"#;

        let (symbol, snapshot) = parse_lob_snapshot(json).unwrap();

        assert_eq!(symbol, Symbol("ETHUSDC".to_string()));
        assert_eq!(snapshot.sequence, SequenceNumber(1000));
        assert_eq!(snapshot.bids.len(), 2);
        assert_eq!(snapshot.asks.len(), 2);

        // Check best bid and ask
        assert_eq!(snapshot.best_bid(), Some(&Price(dec!(2445.12))));
        assert_eq!(snapshot.best_ask(), Some(&Price(dec!(2445.13))));
    }

    #[test]
    fn test_parse_lob_delta() {
        let json = r#"{
            "type": "delta",
            "venue": "binance",
            "symbol": "ETHUSDC",
            "seq": 1001,
            "ts": "2025-01-01T10:00:00.050Z",
            "bids": [
                ["u", "2445.12", "3.0000"]
            ],
            "asks": [
                ["n", "2445.15", "1.00"]
            ]
        }"#;

        let (symbol, delta) = parse_lob_delta(json).unwrap();

        assert_eq!(symbol, Symbol("ETHUSDC".to_string()));
        assert_eq!(delta.sequence, SequenceNumber(1001));
        assert_eq!(delta.updates.len(), 2);

        // Check updates
        let bid_update = &delta.updates[0];
        assert_eq!(bid_update.side, Side::Bid);
        assert_eq!(bid_update.price, Price(dec!(2445.12)));
        assert_eq!(bid_update.quantity, Quantity(dec!(3.0000)));
        assert_eq!(bid_update.action, UpdateAction::Update);

        let ask_update = &delta.updates[1];
        assert_eq!(ask_update.side, Side::Ask);
        assert_eq!(ask_update.price, Price(dec!(2445.15)));
        assert_eq!(ask_update.quantity, Quantity(dec!(1.00)));
        assert_eq!(ask_update.action, UpdateAction::Update);
    }

    #[test]
    fn test_parse_amm_pool() {
        let json = r#"{
            "pool": "0xPOOL",
            "venue": "uniswap_v3",
            "pair": "WETH/USDC",
            "fee": 5,
            "ts": "2025-01-01T10:00:00.000Z",
            "reserves": {
                "token0": "WETH",
                "token1": "USDC",
                "amount0": "1000.0000",
                "amount1": "2445000.00"
            },
            "sqrtPriceX96": "79228162514264337593543950336"
        }"#;

        let (address, update) = parse_amm_pool(json).unwrap();

        assert_eq!(address, PoolAddress("0xPOOL".to_string()));
        assert_eq!(update.fee_tier, FeeTier(5));
        assert_eq!(update.reserves.token0, dec!(1000.0000));
        assert_eq!(update.reserves.token1, dec!(2445000.00));
        assert!(update.sqrt_price.is_some());

        // Check implied price from reserves
        let implied_price = update.implied_mid_from_reserves();
        assert_eq!(implied_price, Price(dec!(2445.0))); // 2445000 / 1000
    }

    #[test]
    fn test_parse_delta_delete_action() {
        let json = r#"{
            "type": "delta",
            "venue": "binance",
            "symbol": "ETHUSDC",
            "seq": 1002,
            "ts": "2025-01-01T10:00:00.100Z",
            "bids": [
                ["d", "2445.11", "0"]
            ],
            "asks": []
        }"#;

        let (_, delta) = parse_lob_delta(json).unwrap();

        assert_eq!(delta.updates.len(), 1);
        let update = &delta.updates[0];
        assert_eq!(update.action, UpdateAction::Delete);
        assert!(update.quantity.is_zero());
    }

    #[test]
    fn test_invalid_json_format() {
        let json = r#"{"invalid": "format"}"#;
        let result = parse_lob_snapshot(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_decimal_parsing() {
        let json = r#"{
            "type": "snapshot",
            "venue": "binance",
            "symbol": "ETHUSDC",
            "seq": 1000,
            "ts": "2025-01-01T10:00:00.000Z",
            "bids": [
                ["invalid_price", "3.1005"]
            ],
            "asks": []
        }"#;

        let result = parse_lob_snapshot(json);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid bid price"));
    }
}
