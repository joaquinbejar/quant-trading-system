//! Domain layer containing core business logic and entities
//!
//! This module contains the core domain entities and business logic for the quantitative
//! trading system, including order books, AMM pools, arbitrage detection, and market events.

/// AMM pool implementations and utilities
pub mod amm_pool;
/// Arbitrage detection and opportunity analysis
pub mod arbitrage;
/// Market events and data structures
pub mod events;
/// Market data management and aggregation
pub mod market_data;
/// Order book implementations
pub mod order_book;
/// Core types and primitives
pub mod types;

pub use events::*;
pub use market_data::*;
pub use types::*;

pub use amm_pool::{AMMPool, AMMPoolType, PriceImpactResult, ThreadSafeAMMPool, TokenIndex};
pub use arbitrage::{ArbitrageDetector, ArbitrageOpportunity, Venue};
pub use order_book::{OrderBookL2, ThreadSafeOrderBook};
