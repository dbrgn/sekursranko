use std::thread;
use std::fs::File;
use std::io::Write;

use hyper::Server;
use hyper::rt::{run as hyper_run, Future};
use reqwest::{Client, Method, header};
use tempfile::{self, TempDir};

use sekursranko::{BackupService, ServerConfig};

struct TestServer {
    handle: thread::JoinHandle<()>,
    base_url: String,
    backup_dir: TempDir,
    config: ServerConfig,
}

impl TestServer {
    /// Create a new test server instance.
    fn new() -> Self {
        let backup_dir = tempfile::Builder::new()
                .prefix("sekursranko-test")
                .tempdir().expect("Could not create temporary backup directory");
        let config = ServerConfig {
            max_backup_bytes: 524288,
            retention_days: 180,
            backup_dir: backup_dir.path().to_path_buf(),
            io_threads: 4,
        };

        let addr = ([127, 0, 0, 1], 0).into();
        let service = BackupService::new(config.clone());
        let server = Server::bind(&addr).serve(service);
        let port = server.local_addr().port();
        let handle = thread::spawn(move || {
            hyper_run(server.map_err(|e| eprintln!("Server error: {}", e)));
        });
        let base_url = format!("http://127.0.0.1:{}", port);

        TestServer { handle, base_url, backup_dir, config }
    }
}

macro_rules! user_agent_required {
    ($name:ident, $url:expr) => {
        #[test]
        fn $name() {
            let TestServer { base_url, .. } = TestServer::new();
            let client = Client::new();
            let res = client
                .get(&format!("{}{}", base_url, $url))
                .send()
                .unwrap();
            assert_eq!(res.status().as_u16(), 400);
        }
    }
}

user_agent_required!(user_agent_required_index, "/");
user_agent_required!(user_agent_required_config, "/config");
user_agent_required!(user_agent_required_backup_download, "/backups/abcd1234");

macro_rules! method_not_allowed {
    ($name:ident, $method:expr, $url:expr) => {
        #[test]
        fn $name() {
            let TestServer { base_url, .. } = TestServer::new();
            let client = Client::new();
            let res = client
                .request($method, &format!("{}{}", base_url, $url))
                .header(header::USER_AGENT, "Threema")
                .send()
                .unwrap();
            assert_eq!(res.status().as_u16(), 405);
        }
    }
}

method_not_allowed!(method_not_allowed_index_post, Method::POST, "/");
method_not_allowed!(method_not_allowed_index_delete, Method::DELETE, "/");
method_not_allowed!(method_not_allowed_config_delete, Method::DELETE, "/config");
method_not_allowed!(method_not_allowed_config_put, Method::PUT, "/config");
method_not_allowed!(method_not_allowed_config_post, Method::POST, "/config");

#[test]
fn index_ok() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let mut res = client
        .get(&base_url)
        .header(header::USER_AGENT, "A Threema B")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(text, "Sekurŝranko 0.1.0");
}

#[test]
fn config_require_json() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let mut res = client
        .get(&format!("{}/config", base_url))
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(text, "{\"detail\": \"Invalid accept header\"}");
}

#[test]
fn backup_download_require_octet_stream() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut res = client
        .get(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/json")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(text, "{\"detail\": \"Invalid accept header\"}");
}

#[test]
fn backup_download_not_found() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut res = client
        .get(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 404);
    assert_eq!(text, "");
}

#[test]
fn backup_download_ok() {
    let TestServer { base_url, backup_dir, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut file = File::create(backup_dir.path().join(backup_id)).unwrap();
    file.write_all(b"tre sekura").unwrap();
    let mut res = client
        .get(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::ACCEPT, "application/octet-stream")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(text, "tre sekura");
}

#[test]
fn backup_upload_require_octet_stream() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut res = client
        .put(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/json")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(text, "{\"detail\": \"Invalid content-type header\"}");
}

#[test]
fn backup_upload_invalid_backup_id() {
    let TestServer { base_url, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789gggggg";
    let mut res = client
        .put(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(text, "{\"detail\": \"Invalid backup ID\"}");
}

/// Request with body that is exactly max bytes large (according to
/// content-length header).
#[test]
fn backup_upload_payload_not_too_large() {
    let TestServer { base_url, config, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut res = client
        .put(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, format!("{}", config.max_backup_bytes))
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_ne!(res.status().as_u16(), 413);
    assert_ne!(text, "{\"detail\": \"Backup is too large\"}");
}

/// Request with body that is a byte too large (according to content-length
/// header).
#[test]
fn backup_upload_payload_too_large() {
    let TestServer { base_url, config, .. } = TestServer::new();
    let client = Client::new();
    let backup_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let mut res = client
        .put(&format!("{}/backups/{}", base_url, backup_id))
        .header(header::USER_AGENT, "Threema")
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CONTENT_LENGTH, format!("{}", config.max_backup_bytes + 1))
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 413);
    assert_eq!(text, "{\"detail\": \"Backup is too large\"}");
}
