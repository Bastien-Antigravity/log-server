//! Log Server Main Binary
//!
//! Centralized logging server that handles both TCP socket (Cap'n Proto)
//! and gRPC log messages with ordered file writing and rotation.


use log_server::core::log_server::LogServer;
use log_server::utils::terminal_ui::print_internal_log;

//================================================================
fn main() {
    let ac = match microservice_toolbox::config::load_config("standalone") {
        Ok(ac) => ac,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(1);
        }
    };

    let name = ac.cli_args.name.as_deref().unwrap_or("log-server");
    let default_host = ac.cli_args.host.as_deref().unwrap_or("127.0.0.1");
    let listen_addr = ac.get_listen_addr(name).unwrap_or_else(|_| format!("{}:9020", default_host));
    
    let default_grpc_host = ac.cli_args.grpc_host.as_deref().unwrap_or(default_host);
    let grpc_listen_addr = ac.get_grpc_listen_addr(name).unwrap_or_else(|_| format!("{}:9021", default_grpc_host));
    let enable_grpc = true;

    // Parse host and port from listen_addr
    let addr_parts: Vec<&str> = listen_addr.split(':').collect();
    let host = addr_parts[0];
    let port = addr_parts[1].parse::<u16>().unwrap_or(9020);

    // Parse grpc_port from grpc_listen_addr
    let grpc_parts: Vec<&str> = grpc_listen_addr.split(':').collect();
    let grpc_port = grpc_parts[1].parse::<u16>().unwrap_or(9021);

    print_internal_log(
        "INFO",
        name,
        "main.rs",
        "41",
        &format!("{name} : starting log server..."),
    );
    if enable_grpc {
        print_internal_log("INFO", name, "main.rs", "45", &format!("{name} : gRPC server enabled"));
    }

    // Run the server
    if let Err(e) = run_server(name, host, port, grpc_port, enable_grpc) {
        print_internal_log(
            "ERROR",
            name,
            "main.rs",
            "54",
            &format!("{name} : server starting failed - {e}"),
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
