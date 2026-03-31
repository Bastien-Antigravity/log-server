// tools/build.rs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Go up one level from tools/ to project root, then into src/logger_proto/
    tonic_build::compile_protos("proto/log_service.proto")?;
    Ok(())
}