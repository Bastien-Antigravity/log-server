//! ESSENTIAL PROCESS:
//! Log Server main executable entry point. Bootstraps the distributed runtime,
//! resolves network addresses via microservice-toolbox, and initializes TCP
//! and gRPC logging daemons.
//!
//! DATA FLOW:
//! 1. CLI / Config slice -> load_config("standalone")
//! 2. LogServer::new() -> LogServer::run()
//!
//! KEY PARAMETERS:
//! - standalone.yaml / local config: Listen host, TCP port (9020), gRPC port (9021).
//! - APP_VERSION: Injected compilation semver from build.rs.

use log_server::facade::log_server::LogServer;
use log_server::line_str;
use log_server::utils::terminal_ui::print_internal_log;

pub const VERSION: &str = env!("APP_VERSION");

fn main() {
    let ac = match microservice_toolbox::config::load_config("standalone") {
        Ok(ac) => ac,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    let name = ac.cli_args.name.as_deref().unwrap_or("log_server");
    let identity = name.to_string();

    let default_host = ac.cli_args.host.as_deref().unwrap_or("0.0.0.0");
    let default_port = ac.cli_args.port.unwrap_or(9020);
    let listen_addr = ac
        .get_listen_addr(name)
        .unwrap_or_else(|_| format!("{}:{}", default_host, default_port));

    print_internal_log(
        "INFO",
        &identity,
        "main.rs",
        "main",
        line_str!(),
        &format!("starting log server (version {}) with profile: {}", VERSION, ac.profile),
    );

    let default_grpc_host = ac.cli_args.grpc_host.as_deref().unwrap_or(default_host);
    let default_grpc_port = ac.cli_args.grpc_port.unwrap_or(default_port + 1);
    let grpc_listen_addr = ac
        .get_grpc_listen_addr(name)
        .unwrap_or_else(|_| format!("{}:{}", default_grpc_host, default_grpc_port));

    let enable_grpc = true;

    // Safely parse host and port from listen_addr [LS-5]
    let (host, port) = match listen_addr.rsplit_once(':') {
        Some((h, p)) => (
            if h.is_empty() { default_host } else { h },
            p.parse::<u16>().unwrap_or(default_port),
        ),
        None => (default_host, default_port),
    };

    // Safely parse grpc_port from grpc_listen_addr [LS-5]
    let grpc_port = match grpc_listen_addr.rsplit_once(':') {
        Some((_, p)) => p.parse::<u16>().unwrap_or(default_grpc_port),
        None => default_grpc_port,
    };

    if enable_grpc {
        print_internal_log(
            "INFO",
            &identity,
            "main.rs",
            "main",
            line_str!(),
            &format!("Log Bridge enabled"),
        );
    }

    // Run the server
    if let Err(e) = run_server(&identity, host, port, grpc_port, enable_grpc) {
        print_internal_log(
            "ERROR",
            &identity,
            "main.rs",
            "main",
            line_str!(),
            &format!("server starting failed - {e}"),
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
