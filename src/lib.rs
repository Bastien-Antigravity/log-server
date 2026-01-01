//! Log Server Library
//!
//! Provides centralized logging server functionality for both
//! TCP socket (Cap'n Proto) and gRPC log messages.

pub mod core;
pub mod network;
pub mod common;
pub mod logger_capnp;
pub mod utils;

// Re-export main components
pub use core::servers::LogServer;
pub use common::config::ServerConfig;
