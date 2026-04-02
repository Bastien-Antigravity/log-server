//! Log Server Library
//!
//! Provides centralized logging server functionality for both
//! TCP socket (Cap'n Proto) and gRPC log messages.

pub mod core;
pub mod servers;
pub mod config;
pub mod protocols;
pub mod models;
pub mod transport;
pub mod utils;

// Re-export main components
pub use core::log_server::LogServer;
pub use config::config::Config;
