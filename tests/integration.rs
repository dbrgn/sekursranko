use std::{
    env,
    fs::File,
    io::{Read, Write},
    net::TcpStream,
    os::unix::fs::PermissionsExt,
    sync::Once,
    thread,
    time::{Duration, Instant},
};

use reqwest::{
    Method,
    blocking::{Client, Response},
    header,
};
use tempfile::{self, TempDir};

use sekursranko::ServerConfig;

static LOGGER_INIT: Once = Once::new();

struct TestServer {
    _handle: thread::JoinHandle<()>,
    base_url: String,
    backup_dir: TempDir,
    config: ServerConfig,
}

impl TestServer {
    /// Create a new test server instance.
    fn new() -> Self {
        // Initialize logger
        LOGGER_INIT.call_once(|| {
            let filter = env::var("RUST_LOG")
                .ok()
                .filter(|v| !v.is_empty())
                .unwrap_or_else(|| "sekursranko=error".into());
            env_logger::Builder::new().parse_filters(&filter).init();
        });

        // Create backup tmpdir
        let backup_dir = tempfile::Builder::new()
            .prefix("sekursranko-test")
            .tempdir()
            .expect("Could not create temporary backup directory");

        // Create config object
        let config = ServerConfig {
            max_backup_bytes: 524_288,
            retention_days: 180,
            backup_dir: backup_dir.path().to_path_buf(),
            listen_on: "-integrationtest-".to_string(),
            allow_browser: None,
        };

        // Run server
        let server_config = config.clone();
        let (port_tx, port_rx) = std::sync::mpsc::channel();
        let handle = thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async move {
                let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                    .await
                    .expect("Could not bind test server");
                let port = listener.local_addr().unwrap().port();

                // Using port 0 when binding, we can know the final port only
                // once the server starts. Therefore, we use a channel to send
                // the final port outside this thread.
                port_tx.send(port).unwrap();

                // Run server until completion
                sekursranko::serve(listener, server_config).await;
            });
        });
        let port = port_rx.recv().unwrap();
        let base_url = format!("http://127.0.0.1:{}", port);

        TestServer {
            _handle: handle,
            base_url,
            backup_dir,
            config,
        }
    }
}

/// Return the value of a response header, if present.
fn header_value(res: &Response, name: header::HeaderName) -> Option<&str> {
    res.headers().get(name).map(|v| v.to_str().unwrap())
}

macro_rules! user_agent_required {
    ($name:ident, $url:expr) => {
        #[test]
        fn $name() {
            let TestServer { base_url, .. } = TestServer::new();
            let res = Client::new()
                .get(&format!("{}{}", base_url, $url))
                .send()
                .unwrap();
            assert_eq!(res.status().as_u16(), 400);
        }
    };
}

user_agent_required!(user_agent_required_index, "/");
user_agent_required!(user_agent_required_config, "/config");
user_agent_required!(user_agent_required_backup_download, "/backups/abcd1234");

macro_rules! method_not_allowed {
    ($name:ident, $method:expr, $url:expr) => {
        #[test]
        fn $name() {
            let TestServer { base_url, .. } = TestServer::new();
            let res = Client::new()
                .request($method, &format!("{}{}", base_url, $url))
                .header(header::USER_AGENT, "Threema")
                .send()
                .unwrap();
            assert_eq!(res.status().as_u16(), 405);
        }
    };
}

method_not_allowed!(method_not_allowed_index_post, Method::POST, "/");
method_not_allowed!(method_not_allowed_index_delete, Method::DELETE, "/");
method_not_allowed!(method_not_allowed_config_put, Method::PUT, "/config");
method_not_allowed!(method_not_allowed_config_post, Method::POST, "/config");
method_not_allowed!(method_not_allowed_config_delete, Method::DELETE, "/config");
method_not_allowed!(
    method_not_allowed_backup_post,
    Method::POST,
    "/backups/abcd1234"
);

#[test]
fn index_ok() {
    let TestServer { base_url, .. } = TestServer::new();
    let res = Client::new()
        .get(&base_url)
        .header(header::USER_AGENT, "A Threema B")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("text/plain; charset=utf-8")
    );
    assert_eq!(
        header_value(&res, header::X_CONTENT_TYPE_OPTIONS),
        Some("nosniff")
    );
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, format!("Sekurŝranko {}", env!("CARGO_PKG_VERSION")));
}

#[test]
fn config_ok() {
    let TestServer { base_url, .. } = TestServer::new();
    let res = Client::new()
        .get(format!("{}/config", base_url))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/json")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("application/json")
    );
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"maxBackupBytes\":524288,\"retentionDays\":180}");
}

#[test]
fn config_require_json() {
    let TestServer { base_url, .. } = TestServer::new();
    let res = Client::new()
        .get(format!("{}/config", base_url))
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("application/json")
    );
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Invalid accept header\"}");
}

#[test]
fn backup_download_require_octet_stream() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .get(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/json")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Invalid accept header\"}");
}

#[test]
fn backup_download_not_found() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .get(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 404);
    assert_eq!(header_value(&res, header::CONTENT_TYPE), None);
    assert_eq!(
        header_value(&res, header::X_CONTENT_TYPE_OPTIONS),
        Some("nosniff")
    );
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");
}

#[test]
fn backup_download_not_found_head() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .head(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 404);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");
}

#[test]
fn backup_download_ok() {
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut file = File::create(backup_dir.path().join(backup_id)).unwrap();
    file.write_all(b"tre sekura").unwrap();
    let res = Client::new()
        .get(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("application/octet-stream")
    );
    assert_eq!(header_value(&res, header::CONTENT_LENGTH), Some("10"));
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "tre sekura");
}

#[test]
fn backup_download_present_head() {
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut file = File::create(backup_dir.path().join(backup_id)).unwrap();
    file.write_all(b"tre sekura").unwrap();
    let res = Client::new()
        .head(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("application/octet-stream")
    );
    assert_eq!(header_value(&res, header::CONTENT_LENGTH), Some("10"));
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");
}

#[test]
fn backup_upload_require_octet_stream() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .put(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/json")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Invalid content-type header\"}");
}

#[test]
fn backup_upload_invalid_backup_id() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789gggggg";
    let res = Client::new()
        .put(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Invalid backup ID\"}");
}

/// Request with body that is exactly max bytes large (according to
/// content-length header).
#[test]
fn backup_upload_payload_not_too_large() {
    let TestServer {
        base_url, config, ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .put(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_LENGTH,
            format!("{}", config.max_backup_bytes),
        )
        .send()
        .unwrap();
    assert_ne!(res.status().as_u16(), 413);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_ne!(text, "{\"detail\": \"Backup is too large\"}");
}

/// Request with body that is a byte too large (according to content-length
/// header).
#[test]
fn backup_upload_payload_too_large() {
    let TestServer {
        base_url, config, ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = Client::new()
        .put(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_LENGTH,
            format!("{}", config.max_backup_bytes + 1),
        )
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 413);
    assert_eq!(
        header_value(&res, header::CONTENT_TYPE),
        Some("application/json")
    );
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Backup is too large\"}");
}

fn upload_backup(base_url: &str, backup_id: &str, body: Vec<u8>) -> Response {
    Client::new()
        .put(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(body)
        .send()
        .unwrap()
}

/// Successfully create a backup.
#[test]
fn backup_upload_success_created() {
    // Test env
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    assert!(backup_dir.path().exists());

    // Send upload request
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = upload_backup(
        &base_url,
        backup_id,
        b"tiu sekurkopio estas tre sekura!".to_vec(),
    );
    assert_eq!(res.status().as_u16(), 201);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");

    // Verify result
    let backup_file_path = backup_dir.path().join(backup_id);
    assert!(backup_file_path.exists(), "Backup file does not exist");
    assert!(
        backup_file_path.is_file(),
        "Backup file is not a regular file"
    );
    let mut backup_file = File::open(backup_file_path).expect("Could not open backup file");
    let mut buffer = String::new();
    backup_file.read_to_string(&mut buffer).unwrap();
    assert_eq!(buffer, "tiu sekurkopio estas tre sekura!");

    // Ensure restrictive permissions
    let perms = backup_file.metadata().unwrap().permissions();
    assert_eq!(perms.mode(), 0o100_000 | 0o600);
}

/// Successfully update a backup.
#[test]
fn backup_upload_success_updated() {
    // Test env
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    assert!(backup_dir.path().exists());

    // Create existing upload file
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let backup_file_path = backup_dir.path().join(backup_id);
    let mut backup_file = File::create(&backup_file_path).expect("Could not create backup file");
    let _ = backup_file.write(b"sekurkopio antikva").unwrap();

    // Send upload request
    let res = upload_backup(
        &base_url,
        backup_id,
        b"tiu sekurkopio estas tre sekura!".to_vec(),
    );
    assert_eq!(res.status().as_u16(), 204);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");

    // Verify result
    assert!(backup_file_path.exists(), "Backup file does not exist");
    assert!(
        backup_file_path.is_file(),
        "Backup file is not a regular file"
    );
    let mut backup_file = File::open(backup_file_path).expect("Could not open backup file");
    let mut buffer = String::new();
    backup_file.read_to_string(&mut buffer).unwrap();
    assert_eq!(buffer, "tiu sekurkopio estas tre sekura!");
}

/// Return the names of all entries in the backup directory.
fn backup_dir_entries(backup_dir: &TempDir) -> Vec<String> {
    let mut entries: Vec<String> = std::fs::read_dir(backup_dir.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    entries.sort();
    entries
}

/// A successful upload must not leave any temporary files behind.
#[test]
fn backup_upload_success_no_temporary_files() {
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let res = upload_backup(&base_url, backup_id, b"one".to_vec());
    assert_eq!(res.status().as_u16(), 201);
    let res = upload_backup(&base_url, backup_id, b"two".to_vec());
    assert_eq!(res.status().as_u16(), 204);
    assert_eq!(backup_dir_entries(&backup_dir), vec![backup_id.to_string()]);
}

/// An upload that is aborted by the client must not leave any files behind.
#[test]
fn backup_upload_aborted_no_files() {
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    // Send only part of the declared body, then close the connection
    let mut stream = TcpStream::connect(base_url.trim_start_matches("http://")).unwrap();
    write!(
        stream,
        "PUT /backups/{} HTTP/1.1\r\n\
         Host: localhost\r\n\
         User-Agent: Threema\r\n\
         Content-Type: application/octet-stream\r\n\
         Content-Length: 100\r\n\
         \r\n\
         incomplete",
        backup_id
    )
    .unwrap();
    stream.flush().unwrap();

    // Wait until the server has started writing the upload
    let deadline = Instant::now() + Duration::from_secs(5);
    while backup_dir_entries(&backup_dir).is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    drop(stream);

    // Wait until the server has cleaned up
    let deadline = Instant::now() + Duration::from_secs(5);
    while !backup_dir_entries(&backup_dir).is_empty() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(backup_dir_entries(&backup_dir), Vec::<String>::new());
}

#[test]
fn backup_delete_invalid_backup_id() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789gggggg";
    let res = Client::new()
        .delete(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "{\"detail\": \"Invalid backup ID\"}");
}

#[test]
fn backup_delete_not_found() {
    let TestServer { base_url, .. } = TestServer::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789ffffff";
    let res = Client::new()
        .delete(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 404);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");
}

/// Delete a backup.
#[test]
fn backup_delete_success() {
    // Test env
    let TestServer {
        base_url,
        backup_dir,
        ..
    } = TestServer::new();
    assert!(backup_dir.path().exists());

    // Create existing upload file
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let backup_file_path = backup_dir.path().join(backup_id);
    let mut backup_file = File::create(&backup_file_path).expect("Could not create backup file");
    let _ = backup_file.write(b"sekurkopio antikva").unwrap();

    // Ensure file was created
    assert!(backup_file_path.exists() && backup_file_path.is_file());

    // Send delete request
    let res = Client::new()
        .delete(format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 204);
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(text, "");

    // Ensure file was deleted
    assert!(!backup_file_path.exists());
}
