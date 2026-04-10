//! Log Server Main Binary
//!
//! Centralized logging server that handles both TCP socket (Cap'n Proto)
//! and gRPC log messages with ordered file writing and rotation.

use log_server::core::log_server::LogServer;
use log_server::utils::terminal_ui::print_internal_log;
use microservice_toolbox::config::loader::load_config;

//================================================================
fn main() {
    // 1. Initialize Toolbox Config (handles --grpc_port, --grpc_host, --name, etc.)
    let app_config = load_config("standalone");
    let cli = &app_config.cli_args;

    let name = cli.name.as_deref().unwrap_or("log-server");
    let host = cli.host.as_deref().unwrap_or("127.0.0.1");
    let port = cli.port.unwrap_or(9020);
    let grpc_port = cli.grpc_port.unwrap_or(9021);

    // Extract specific flags if needed
    let enable_grpc = cli.extras.get("enable_grpc").is_some_and(|v| v == "true");

    print_internal_log(
        "INFO",
        name,
        "main.rs",
        "38",
        &format!("starting log server (name: {name})"),
    );
    if enable_grpc {
        print_internal_log("DEBUG", name, "main.rs", "42", "gRPC server enabled");
    }

    // Run the server
    if let Err(e) = run_server(name, host, port, grpc_port, enable_grpc) {
        print_internal_log(
            "ERROR",
            name,
            "main.rs",
            "51",
            &format!("server failed - {e}"),
        );
        std::process::exit(1);
    }
}

//-----------------------------------------------------------------------------------------------
/// Main server execution function
fn run_server(
    name: &str,
    host: &str,
    port: u16,
    grpc_port: u16,
    enable_grpc: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let server = LogServer::new(name, host, port, grpc_port, enable_grpc).await?;
        server.run().await
    })
}
