//! ESSENTIAL PROCESS:
//! Log Server main executable entry point. Bootstraps the distributed runtime,
//! resolves network addresses via microservice-toolbox, and initializes TCP
//! and gRPC logging daemons.
//!
//! DATA FLOW:
//! 1. Runtime environment -> resolve_config_profile() -> load_config(profile)
//! 2. LogServer::new() -> LogServer::run()
//!
//! KEY PARAMETERS:
//! - production.yaml / standalone.yaml: Listen host/port (TCP) and grpc_ip/grpc_port (gRPC).
//! - CONFIG_PROFILE / DOCKER_ENV: Active configuration profile selection.

use std::sync::Arc;

use microservice_toolbox::config::loader::AppConfig;

use log_server::config::config::Config;
use log_server::facade::log_server::LogServer;
use log_server::line_str;
use log_server::utils::terminal_ui::print_internal_log;

//-----------------------------------------------------------------------------------------------

fn main() {
    let profile = resolve_config_profile();
    let ac = match microservice_toolbox::config::load_config(&profile) {
        Ok(ac) => ac,
        Err(e) => {
            print_internal_log(
                "CRITICAL",
                "log_server",
                "main.rs",
                "main",
                line_str!(),
                &format!("Error loading configuration: {e}"),
            );
            std::process::exit(1);
        }
    };

    let name = ac.cli_args.name.as_deref().unwrap_or("log_server");
    let identity = name.to_string();
    let version = ac.get_version();
    if version.trim().is_empty() {
        print_internal_log(
            "CRITICAL",
            &identity,
            "main.rs",
            "main",
            line_str!(),
            "Fatal: AppConfig::get_version() returned empty string (missing VERSION.txt)",
        );
        std::process::exit(1);
    }

    let endpoints = match resolve_service_addrs(&ac, name) {
        Ok(addrs) => addrs,
        Err(e) => {
            print_internal_log("CRITICAL", &identity, "main.rs", "main", line_str!(), &e);
            std::process::exit(1);
        }
    };

    print_internal_log(
        "INFO",
        &identity,
        "main.rs",
        "main",
        line_str!(),
        &format!(
            "starting log server (version {}) with profile: {}",
            version,
            ac.profile
        ),
    );

    let enable_grpc = true;
    print_internal_log(
        "INFO",
        &identity,
        "main.rs",
        "main",
        line_str!(),
        "Log Bridge enabled",
    );

    let fsync_enabled = log_server::config::config::parse_fsync_config(&ac);
    let config = Config::new(
        &identity,
        &endpoints.tcp.host,
        endpoints.tcp.port,
        endpoints.grpc.port,
    )
    .with_version(&version)
    .with_grpc_host(&endpoints.grpc.host)
    .with_sync_on_flush(fsync_enabled);
    if let Err(e) = run_server(config, Arc::new(ac), enable_grpc) {
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

#[derive(Debug, PartialEq, Eq)]
struct Endpoint {
    host: String,
    port: u16,
}

#[derive(Debug, PartialEq, Eq)]
struct ServiceEndpoints {
    tcp: Endpoint,
    grpc: Endpoint,
}

//-----------------------------------------------------------------------------------------------

/// Resolve TCP and gRPC endpoints verbatim from `AppConfig`.
fn resolve_service_addrs(ac: &AppConfig, name: &str) -> Result<ServiceEndpoints, String> {
    let capability = name.replace('-', "_");

    let listen_addr = ac
        .get_listen_addr(name)
        .or_else(|_| ac.get_listen_addr(&capability))
        .map_err(|e| format!("failed to resolve TCP listen address for '{name}': {e}"))?;

    let grpc_listen_addr = ac
        .get_grpc_listen_addr(name)
        .or_else(|_| ac.get_grpc_listen_addr(&capability))
        .map_err(|e| format!("failed to resolve gRPC listen address for '{name}': {e}"))?;

    Ok(ServiceEndpoints {
        tcp: parse_host_port(&listen_addr)?,
        grpc: parse_host_port(&grpc_listen_addr)?,
    })
}

//-----------------------------------------------------------------------------------------------

/// Parse a `host:port` (or `[ipv6]:port`) endpoint string verbatim without silent fallbacks.
fn parse_host_port(addr: &str) -> Result<Endpoint, String> {
    let trimmed = addr.trim();
    let (raw_host, raw_port) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| format!("missing ':port' in configured address '{trimmed}'"))?;

    let host = raw_host.trim();
    if host.is_empty() {
        return Err(format!("empty host in configured address '{trimmed}'"));
    }

    let port = raw_port
        .trim()
        .parse::<u16>()
        .map_err(|e| format!("invalid port in configured address '{trimmed}': {e}"))?;

    Ok(Endpoint {
        host: host.to_string(),
        port,
    })
}

//-----------------------------------------------------------------------------------------------

/// Resolves the configuration profile based on `CONFIG_PROFILE` or runtime environment (`DOCKER_ENV=true`).
fn resolve_config_profile() -> String {
    if let Ok(profile) = std::env::var("CONFIG_PROFILE") {
        let trimmed = profile.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    let in_docker = std::env::var("DOCKER_ENV")
        .map(|v| v == "true")
        .unwrap_or(false);

    if in_docker {
        return "production".to_string();
    }

    "standalone".to_string()
}

//-----------------------------------------------------------------------------------------------

/// Main server execution function
fn run_server(
    config: Config,
    app_config: Arc<AppConfig>,
    enable_grpc: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let server = LogServer::new_with_config(config, enable_grpc, Some(app_config)).await?;
        server.run().await
    })
}

//-----------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn test_parse_host_port_preserves_host_verbatim() {
        assert_eq!(
            parse_host_port("log-server:19021").unwrap(),
            Endpoint {
                host: "log-server".to_string(),
                port: 19021,
            }
        );
        assert_eq!(
            parse_host_port("127.0.0.2:19020").unwrap(),
            Endpoint {
                host: "127.0.0.2".to_string(),
                port: 19020,
            }
        );
        assert_eq!(
            parse_host_port("[::1]:19021").unwrap(),
            Endpoint {
                host: "[::1]".to_string(),
                port: 19021,
            }
        );
        assert!(parse_host_port(":19021").is_err());
        assert!(parse_host_port("log-server").is_err());
        assert!(parse_host_port("log-server:bad").is_err());
    }

    //-----------------------------------------------------------------------------------------------

    #[test]
    fn test_resolve_service_addrs_preserves_distinct_grpc_host() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("log_server_main_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let cfg_path = dir.join("production.yaml");
        std::fs::write(
            &cfg_path,
            "capabilities:\n  log_server:\n    ip: 127.0.0.1\n    port: '19020'\n    grpc_ip: log-server\n    grpc_port: '19021'\n",
        )
        .expect("write temp config");

        let prev_cfg_path = std::env::var_os("CONFIG_PATH");
        std::env::set_var("CONFIG_PATH", &cfg_path);
        let ac = microservice_toolbox::config::load_config("production").expect("load temp config");
        match prev_cfg_path {
            Some(v) => std::env::set_var("CONFIG_PATH", v),
            None => std::env::remove_var("CONFIG_PATH"),
        }
        let _ = std::fs::remove_dir_all(&dir);

        let endpoints = resolve_service_addrs(&ac, "log-server").expect("resolve log_server addrs");
        assert_eq!(endpoints.tcp.host, "127.0.0.1");
        assert_eq!(endpoints.tcp.port, 19020);
        assert_eq!(endpoints.grpc.host, "log-server");
        assert_eq!(endpoints.grpc.port, 19021);
    }

    //-----------------------------------------------------------------------------------------------

    #[test]
    fn test_resolve_service_addrs_fails_fast_when_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!("log_server_main_missing_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let cfg_path = dir.join("empty.yaml");
        std::fs::write(&cfg_path, "{}\n").expect("write empty temp config");

        let prev_cfg_path = std::env::var_os("CONFIG_PATH");
        std::env::set_var("CONFIG_PATH", &cfg_path);
        let ac = microservice_toolbox::config::load_config("empty").expect("load empty config");
        match prev_cfg_path {
            Some(v) => std::env::set_var("CONFIG_PATH", v),
            None => std::env::remove_var("CONFIG_PATH"),
        }
        let _ = std::fs::remove_dir_all(&dir);

        assert!(resolve_service_addrs(&ac, "log_server").is_err());
    }
}
