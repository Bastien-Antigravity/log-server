// =============================================================================
// ESSENTIAL PROCESS:
// Build script for log-server. Compiles protobuf schemas via tonic-build for
// gRPC services and resolves dynamic semver from VERSION.txt.
//
// DATA FLOW:
// Reads src/schema/proto/log_service.proto -> generates Rust gRPC bindings.
// Reads VERSION.txt -> exposes APP_VERSION environment variable to compiler.
//
// KEY PARAMETERS:
// - src/schema/proto/log_service.proto: Input proto schema for log bridge.
// - APP_VERSION: Injected cargo rustc environment variable.
// =============================================================================

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Compile gRPC protobuf schemas
    tonic_build::compile_protos("src/schema/proto/log_service.proto")?;

    // 2. Dynamic version resolution
    let version = std::fs::read_to_string("VERSION.txt")
        .or_else(|_| std::fs::read_to_string("../VERSION.txt"))
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=APP_VERSION={}", version.trim());
    println!("cargo:rerun-if-changed=VERSION.txt");
    println!("cargo:rerun-if-changed=../VERSION.txt");
    println!("cargo:rerun-if-changed=src/schema/proto/log_service.proto");

    Ok(())
}
