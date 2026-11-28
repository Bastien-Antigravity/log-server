//! ESSENTIAL PROCESS:
//! Build script for log-server. Compiles protobuf schemas via tonic-build for
//! gRPC services and validates VERSION.txt presence at compile time.
//!
//! DATA FLOW:
//! 1. Reads src/schema/proto/log_service.proto -> generates Rust gRPC bindings.
//! 2. Verifies VERSION.txt exists and is non-empty (fail-fast).
//!
//! KEY PARAMETERS:
//! - src/schema/proto/log_service.proto: Input proto schema for log bridge.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Compile gRPC protobuf schemas
    tonic_build::compile_protos("src/schema/proto/log_service.proto")?;

    // 2. Fail-fast compile-time verification of VERSION.txt
    let version = std::fs::read_to_string("VERSION.txt")
        .or_else(|_| std::fs::read_to_string("../VERSION.txt"))
        .expect("Fatal: VERSION.txt is missing at compile time");
    assert!(
        !version.trim().is_empty(),
        "Fatal: VERSION.txt is empty at compile time"
    );
    println!("cargo:rerun-if-changed=VERSION.txt");
    println!("cargo:rerun-if-changed=../VERSION.txt");
    println!("cargo:rerun-if-changed=src/schema/proto/log_service.proto");

    Ok(())
}
