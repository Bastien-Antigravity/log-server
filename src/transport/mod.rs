//! ESSENTIAL PROCESS:
//! Transport communication layer module for framed TCP sockets.
//!
//! DATA FLOW:
//! 1. Provides SafeSocket primitives to server listeners.
//!
//! KEY PARAMETERS:
//! - safe_socket: Framed TCP socket implementation.

pub mod safe_socket;
