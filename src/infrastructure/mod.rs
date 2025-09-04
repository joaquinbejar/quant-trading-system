//! Infrastructure layer providing data ingestion, metrics, and parsing capabilities
//!
//! This module contains the infrastructure components that support the domain layer,
//! including stream ingestion, metrics collection, and data parsing utilities.

/// Stream ingestion utilities for real-time market data
pub mod ingestion;
/// Metrics collection and export functionality
pub mod metrics;
/// Data parsing utilities for various market data formats
pub mod parsers;

pub use ingestion::*;
pub use parsers::*;
