use capnp::serialize_packed;
use std::fs;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};

use log_server::core::log_server::LogServer;
use log_server::utils::helpers::get_exec_parent_dir;

// Include the generated gRPC code for the test client
pub mod log_service {
    tonic::include_proto!("logservice");
}

use log_service::log_service_client::LogServiceClient;
use log_service::LogRequest;

#[tokio::test]
async fn test_full_log_pipeline() {
    // 1. Setup - get the log file path relative to the test executable
    let log_dir = get_exec_parent_dir().join("logs");
    let log_file_path = log_dir.join("_main.log");

    // Clean start
    if log_file_path.exists() {
        let _ = fs::remove_file(&log_file_path);
    }

    let name = "test-server";
    let host = "0.0.0.0";
    let client_host = "127.0.0.1";
    let tcp_port = 12920;
    let grpc_port = 12921;

    // 1. Start the server in a background task
    let server = LogServer::new(name, host, tcp_port, grpc_port, true)
        .await
        .unwrap();
    let server_handle = tokio::spawn(async move {
        // Run until aborted
        let _ = server.run().await;
    });

    // Wait for server to start - give it enough time to bind ports
    sleep(Duration::from_millis(3500)).await;

    // 2. Send a TCP Cap'n Proto message
    send_test_tcp_message(client_host, tcp_port).await;

    // 3. Send a gRPC message
    send_test_grpc_message(client_host, grpc_port).await;

    // 4. Wait for messages to be processed and flushed
    sleep(Duration::from_millis(2000)).await;

    // 5. Verify the log file content
    let mut contents = String::new();
    let mut success = false;
    for _ in 0..15 {
        if log_file_path.exists() {
            contents = fs::read_to_string(&log_file_path).unwrap();
            if contents.contains("TCP_TEST_MESSAGE") && contents.contains("GRPC_TEST_MESSAGE") {
                success = true;
                break;
            }
        }
        sleep(Duration::from_millis(1000)).await;
    }

    assert!(
        success,
        "Log file should contain both messages. Path: {:?}\nContents last read:\n{}",
        log_file_path, contents
    );

    // Cleanup
    server_handle.abort();
}

async fn send_test_tcp_message(host: &str, port: u16) {
    let mut stream = TcpStream::connect(format!("{}:{}", host, port))
        .await
        .expect("Failed to connect to TCP server");

    let mut message = ::capnp::message::Builder::new_default();
    {
        let mut builder =
            message.init_root::<::log_server::protocols::capnp::logger_msg::logger_msg::Builder>();
        builder.set_message("TCP_TEST_MESSAGE");
        builder.set_level(::log_server::protocols::capnp::logger_msg::Level::Info);
        builder.set_logger_name("tcp-client");
        builder.set_timestamp("2026-04-02T10:00:00Z");
        builder.set_hostname("localhost");
    }

    let mut buffer = Vec::new();
    serialize_packed::write_message(&mut buffer, &message).unwrap();

    // Header: 4 bytes BE length (matching SafeSocket protocol)
    let len_prefix = (buffer.len() as u32).to_be_bytes();
    stream.write_all(&len_prefix).await.unwrap();
    stream.write_all(&buffer).await.unwrap();
    stream.flush().await.unwrap();
}
async fn send_test_grpc_message(host: &str, port: u16) {
    // Try to connect with retries
    let mut channel = None;
    for attempt in 0..5 {
        match tonic::transport::Channel::from_shared(format!("http://{host}:{port}"))
            .unwrap()
            .connect()
            .await
        {
            Ok(c) => {
                channel = Some(c);
                break;
            }
            Err(_) if attempt < 4 => {
                sleep(Duration::from_millis(500)).await;
            }
            Err(e) => panic!("Failed to connect to gRPC server after retries: {e}"),
        }
    }

    let channel = channel.unwrap();

    let mut client = LogServiceClient::new(channel);

    let request = tonic::Request::new(LogRequest {
        timestamp: "2026-04-02T10:00:00Z".into(),
        hostname: "localhost".into(),
        logger_name: "grpc-client".into(),
        level: 3, // INFO
        module: "test".into(),
        filename: "test.rs".into(),
        function_name: "test".into(),
        line_number: "1".into(),
        message: "GRPC_TEST_MESSAGE".into(),
        ..Default::default()
    });

    client
        .log_message(request)
        .await
        .expect("Failed to send gRPC message");
}
