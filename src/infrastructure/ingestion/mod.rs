//! Stream ingestion utilities for real-time market data processing
//!
//! This module provides components for ingesting and processing real-time market data streams
//! with comprehensive statistics tracking and error handling.

/// Stream ingestion with statistics and error handling
pub mod stream_ingester;

pub use stream_ingester::*;
