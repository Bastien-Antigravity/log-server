//! Log Server Main Binary
//!
//! Centralized logging server that handles both TCP socket (Cap'n Proto)
//! and gRPC log messages with ordered file writing and rotation.

use clap::{Arg, Command};
use log_server::core::log_server::LogServer;
use log_server::utils::terminal_ui::print_internal_log;

//================================================================
fn main() {
    let matches = Command::new("log-server")
        .arg(Arg::new("name").long("name").default_value("log-server"))
        .arg(Arg::new("host").long("host").default_value("127.0.0.1"))
        .arg(Arg::new("port").long("port").default_value("9020"))
        .arg(
            Arg::new("grpc_port")
                .long("grpc_port")
                .default_value("9021"),
        )
        .arg(
            Arg::new("enable_grpc")
                .long("enable_grpc")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let name = matches.get_one::<String>("name").unwrap();
    let host = matches.get_one::<String>("host").unwrap();
    let port = matches
        .get_one::<String>("port")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let grpc_port = matches
        .get_one::<String>("grpc_port")
        .unwrap()
        .parse::<u16>()
        .unwrap();
    let enable_grpc = matches.get_flag("enable_grpc");

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
