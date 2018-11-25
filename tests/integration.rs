extern crate hyper;
extern crate reqwest;
extern crate rustysafe;

use std::thread;

use hyper::Server;
use hyper::rt::Future;
use hyper::service::service_fn;
use reqwest::{Client, header};

/// Create a new test server instance and return the bound URL.
fn testserver() -> (thread::JoinHandle<()>, String) {
    let addr = ([127, 0, 0, 1], 0).into();
    let service = || service_fn(rustysafe::handler);
    let server = Server::bind(&addr).serve(service);
    let port = server.local_addr().port();
    let handle = thread::spawn(move || {
        hyper::rt::run(server.map_err(|e| eprintln!("Server error: {}", e)));
    });
    (handle, format!("http://127.0.0.1:{}", port))
}

macro_rules! user_agent_required {
    ($name:ident, $url:expr) => {
        #[test]
        fn $name() {
            let (_handle, base_url) = testserver();
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

#[test]
fn index_ok() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    let mut res = client
        .get(&base_url)
        .header(header::USER_AGENT, "Foo Threema Bar")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(text, "rustysafe 0.1.0");
}

#[test]
fn config_require_json() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    let mut res = client
        .get(&format!("{}/config", base_url))
        .header(header::USER_AGENT, "Foo Threema Bar")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 400);
    assert_eq!(text, "{\"detail\": \"Invalid accept header\"}");
}

#[test]
fn config_ok() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    let mut res = client
        .get(&format!("{}/config", base_url))
        .header(header::USER_AGENT, "Foo Threema Bar")
        .header(header::ACCEPT, "application/json")
        .send()
        .unwrap();
    let text = res.text().unwrap();
    println!("{}", text);
    assert_eq!(res.status().as_u16(), 200);
    assert_eq!(text, "{\"maxBackupBytes\": 524288, \"retentionDays\": 180}");
}
