//! ESSENTIAL PROCESS:
//! Main log server orchestrator coordinating TCP socket (Cap'n Proto) and
//! high-throughput gRPC (HTTP/2) with single-authority FIFO asynchronous file writing,
//! rotation, and real-time broadcast streaming.
//!
//! DATA FLOW:
//! 1. TCP/gRPC clients -> writer_tx FIFO channel
//! 2. WriterTask sequences and flushes ordered batches to files and updates LogRingBuffer
//! 3. gRPC StreamLogs -> ring_buffer broadcast -> real-time HTTP/2 stream
//!
//! KEY PARAMETERS:
//! - name: Microservice identity ("log_server").
//! - host / port: TCP socket binding (default 0.0.0.0:9020).
//! - grpc_port: gRPC bridge endpoint (default 9021).

use microservice_toolbox::config::loader::AppConfig;
use std::sync::Arc;

use crate::config::config::Config;
use crate::core::ring_buffer::LogRingBuffer;
use crate::facade::log_writer::LogWriter;
use crate::line_str;
use crate::servers::grpc_server::LogBridgeGateway;
use crate::servers::tcp_server::TcpServer;
use crate::utils::terminal_ui::print_internal_log;

/// Main log server orchestrator
pub struct LogServer {
    name: String,
    config: Config,
    writer: Arc<LogWriter>,
    enable_grpc: bool,
    app_config: Option<Arc<AppConfig>>,
}

//-----------------------------------------------------------------------------------------------

impl LogServer {
    /// Create new log server instance
    pub async fn new(
        name: &str,
        host: &str,
        port: u16,
        grpc_port: u16,
        enable_grpc: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let config = Config::new(name, host, port, grpc_port);
        Self::new_with_config(config, enable_grpc, None).await
    }

    //-----------------------------------------------------------------------------------------------

    /// Create new log server instance from a full `Config` and optional active `AppConfig`
    pub async fn new_with_config(
        config: Config,
        enable_grpc: bool,
        app_config: Option<Arc<AppConfig>>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Resolve fsync durability from AppConfig or environment if available
        if let Some(ref ac) = app_config {
            let fsync_enabled = crate::config::config::parse_fsync_config(ac);
            config.set_sync_on_flush(fsync_enabled);
            microservice_toolbox::wiretap::configure_from_config(&config.name, ac);

            // Wire live config hot-reload to update hardware durability barrier (fsync) on the fly
            let sync_flag = config.sync_on_flush.clone();
            let svc_name = config.name.clone();
            let cap_key = svc_name.replace('-', "_");
            ac.on_live_conf_update(move |val| {
                let raw_fsync = val
                    .get(&cap_key)
                    .and_then(|s| s.get("fsync"))
                    .or_else(|| val.get("log_server").and_then(|s| s.get("fsync")))
                    .or_else(|| {
                        val.get("capabilities").and_then(|c| {
                            c.get(&cap_key)
                                .and_then(|s| s.get("fsync"))
                                .or_else(|| c.get("log_server").and_then(|s| s.get("fsync")))
                                .or_else(|| c.get(format!("{cap_key}.fsync")))
                                .or_else(|| c.get("log_server.fsync"))
                        })
                    });
                if let Some(v) = raw_fsync {
                    let enabled = v.as_bool().unwrap_or_else(|| {
                        v.as_str()
                            .map(|s| {
                                let lower = s.trim().to_lowercase();
                                lower == "true" || lower == "1" || lower == "yes"
                            })
                            .unwrap_or(false)
                    });
                    sync_flag.store(enabled, std::sync::atomic::Ordering::Relaxed);
                    print_internal_log(
                        "INFO",
                        &svc_name,
                        "log_server.rs",
                        "on_live_conf_update",
                        line_str!(),
                        &format!("live config hot-reload applied: fsync={enabled}"),
                    );
                }
            });
        } else if let Ok(val) = std::env::var("LS_FSYNC") {
            let val = val.trim().to_lowercase();
            let fsync_enabled = val == "true" || val == "1" || val == "yes";
            config.set_sync_on_flush(fsync_enabled);
        }

        // Create resolved log directory (LOG_DIR -> DOCKER_ENV=true -> <exec_parent>/logs)
        let log_dir = crate::utils::helpers::resolve_log_dir();
        if let Some(log_dir_str) = log_dir.to_str() {
            crate::utils::helpers::create_log_folder(log_dir_str)?;
        }

        // Initialize writer with matching shared sync_on_flush atomic
        let writer_config = crate::facade::log_writer::WriterConfig {
            sync_on_flush: config.sync_on_flush.clone(),
            ..Default::default()
        };
        let writer = Arc::new(LogWriter::with_config(writer_config).await?);

        Ok(Self {
            name: config.name.clone(),
            config,
            writer,
            enable_grpc,
            app_config,
        })
    }

    //-----------------------------------------------------------------------------------------------

    /// Dynamic runtime control for hardware durability barrier (fsync).
    pub fn set_sync_on_flush(&self, enabled: bool) {
        self.config.set_sync_on_flush(enabled);
        self.writer.set_sync_on_flush(enabled);
    }

    //-----------------------------------------------------------------------------------------------

    /// Query active hardware durability barrier (fsync) state.
    pub fn is_sync_on_flush(&self) -> bool {
        self.config.is_sync_on_flush()
    }

    //-----------------------------------------------------------------------------------------------

    /// Run the log server with all components
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            line_str!(),
            "starting server components. .  .",
        );

        // In-memory circular ring buffer for real-time log querying and broadcast streaming
        let ring_buffer = Arc::new(LogRingBuffer::new(5000));

        // Start a single writer task for all components to share
        let (writer_tx, mut writer_handle) = self.writer.start_writer_task(ring_buffer.clone());

        // Initialize the internal logger so it can also write to files
        crate::utils::terminal_ui::set_internal_logger(writer_tx.clone());

        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            line_str!(),
            "internal logger initialized - writer(s) ready !",
        );

        let fsync_state = if self.is_sync_on_flush() {
            "enabled (hardware NAND commit)"
        } else {
            "disabled (OS page cache)"
        };
        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            line_str!(),
            &format!("hardware durability barrier (fsync): {fsync_state}"),
        );

        // Start TCP server (always)
        let tcp_server = TcpServer::new(&self.config);
        let tcp_writer_tx = writer_tx.clone();
        let identity = self.name.clone();

        let mut tcp_handle = tokio::spawn(async move {
            if let Err(e) = tcp_server.run(tcp_writer_tx).await {
                print_internal_log(
                    "ERROR",
                    &identity,
                    "log_server.rs",
                    "run",
                    line_str!(),
                    &format!("TCP server error: {e}"),
                );
                Err(e.to_string())
            } else {
                Ok(())
            }
        });

        // Conditionally start gRPC bridge gateway with real-time stream support
        let identity_grpc = self.name.clone();
        let mut grpc_handle = if self.enable_grpc {
            let grpc_server = LogBridgeGateway::new(&self.config);
            let grpc_writer_tx = writer_tx.clone();
            let grpc_ring_buffer = ring_buffer.clone();

            Some(tokio::spawn(async move {
                if let Err(e) = grpc_server.run(grpc_writer_tx, grpc_ring_buffer).await {
                    print_internal_log(
                        "ERROR",
                        &identity_grpc,
                        "log_server.rs",
                        "run",
                        line_str!(),
                        &format!("Log Bridge error: {e}"),
                    );
                    Err(e.to_string())
                } else {
                    Ok(())
                }
            }))
        } else {
            None
        };

        print_internal_log(
            "INFO",
            &self.name,
            "log_server.rs",
            "run",
            line_str!(),
            "all server components started !",
        );

        // Publish exhaustive OpenMFE spec + contracts asynchronously to web-interface
        crate::core::contracts::spawn_web_interface_registration(
            self.config.clone(),
            5000,
            self.app_config.as_deref(),
        );

        // Wait for any server task, writer task, or OS termination signal; then execute unified teardown
        let mut writer_finished = false;
        let outcome: Result<(), String> = tokio::select! {
            res = &mut tcp_handle => {
                match res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(join_err) => Err(join_err.to_string()),
                }
            }
            res = async {
                if let Some(ref mut h) = grpc_handle {
                    h.await
                } else {
                    std::future::pending().await
                }
            } => {
                match res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => Err(e),
                    Err(join_err) => Err(join_err.to_string()),
                }
            }
            res = &mut writer_handle => {
                writer_finished = true;
                match res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(e)) => {
                        print_internal_log(
                            "CRITICAL",
                            &self.name,
                            "log_server.rs",
                            "run",
                            line_str!(),
                            &format!("Fatal disk I/O failure in writer task: {e}"),
                        );
                        Err(e.to_string())
                    }
                    Err(join_err) => {
                        print_internal_log(
                            "CRITICAL",
                            &self.name,
                            "log_server.rs",
                            "run",
                            line_str!(),
                            &format!("Writer task panicked or was aborted: {join_err}"),
                        );
                        Err(join_err.to_string())
                    }
                }
            }
            _ = async {
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{signal, SignalKind};
                    let mut sigterm = signal(SignalKind::terminate()).ok();
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {},
                        _ = async {
                            if let Some(s) = sigterm.as_mut() {
                                s.recv().await;
                            } else {
                                std::future::pending::<()>().await;
                            }
                        } => {},
                    }
                }
                #[cfg(not(unix))]
                {
                    let _ = tokio::signal::ctrl_c().await;
                }
            } => {
                print_internal_log(
                    "INFO",
                    &self.name,
                    "log_server.rs",
                    "run",
                    line_str!(),
                    "shutdown signal received; draining buffered logs and terminating gracefully...",
                );
                Ok(())
            }
        };

        // Unified teardown across ALL exit paths: abort ingress listeners (and their JoinSet client tasks),
        // await task cancellation so all cloned writer_tx handles drop, clear internal logger, and drain writer.
        tcp_handle.abort();
        let _ = (&mut tcp_handle).await;
        if let Some(ref mut h) = grpc_handle {
            h.abort();
            let _ = h.await;
        }
        crate::utils::terminal_ui::clear_internal_logger();
        drop(writer_tx);

        if !writer_finished {
            match writer_handle.await {
                Ok(Ok(())) => {
                    print_internal_log(
                        "INFO",
                        &self.name,
                        "log_server.rs",
                        "run",
                        line_str!(),
                        "clean shutdown complete: all buffered records flushed to disk.",
                    );
                }
                Ok(Err(e)) => {
                    if outcome.is_ok() {
                        return Err(Box::new(e) as Box<dyn std::error::Error>);
                    }
                }
                Err(join_err) => {
                    if outcome.is_ok() {
                        return Err(Box::new(join_err) as Box<dyn std::error::Error>);
                    }
                }
            }
        }

        outcome.map_err(|e| e.into())
    }
}
