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

#[test]
fn headers_required() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    // No headers
    let res = client
        .get(&base_url)
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
}

#[test]
fn accept_json_required() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    // Only user agent, no accept header
    let res = client
        .get(&base_url)
        .header(header::USER_AGENT, "Threema")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
}

#[test]
fn user_agent_required() {
    let (_handle, base_url) = testserver();
    let client = Client::new();
    // Only accept header, no user agent
    let res = client
        .get(&base_url)
        .header(header::ACCEPT, "application/json")
        .send()
        .unwrap();
    assert_eq!(res.status().as_u16(), 400);
}
