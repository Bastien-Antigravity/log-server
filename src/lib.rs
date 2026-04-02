//! Log Server Library
//!
//! Provides centralized logging server functionality for both
//! TCP socket (Cap'n Proto) and gRPC log messages.

pub mod config;
pub mod core;
pub mod models;
pub mod protocols;
pub mod servers;
pub mod transport;
pub mod utils;

// Re-export main components
pub use config::config::Config;
pub use core::log_server::LogServer;
