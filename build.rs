// =============================================================================
// ESSENTIAL PROCESS:
// Dynamic version resolution build script for log-server.
// Reads VERSION.txt from repository root as single source of truth.
//
// DATA FLOW:
// Reads VERSION.txt -> exposes APP_VERSION environment variable to compiler.
// =============================================================================

fn main() {
    let version = std::fs::read_to_string("VERSION.txt")
        .or_else(|_| std::fs::read_to_string("../VERSION.txt"))
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=APP_VERSION={}", version.trim());
    println!("cargo:rerun-if-changed=VERSION.txt");
    println!("cargo:rerun-if-changed=../VERSION.txt");
}
