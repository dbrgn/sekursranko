extern crate futures;
extern crate hyper;
extern crate log;

use futures::future;
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::Future;
use log::warn;

static VERSION: &str = env!("CARGO_PKG_VERSION");

/// The server configuration.
/// TODO: Serialize
struct ServerConfig {
    max_backup_bytes: u32,
    retention_days: u32,
}

// TODO: Make configurable
static SERVER_CONFIG: ServerConfig = ServerConfig {
    max_backup_bytes: 524288,
    retention_days: 180,
};

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

// "application/json"
macro_rules! require_accept_starts_with {
    ($req:expr, $resp:expr, $accept:expr) => {
        match $req.headers().get(header::ACCEPT).and_then(|v| v.to_str().ok()) {
            Some(accept) if accept.starts_with($accept) => {},
            _ => {
                warn!("Received request without valid accept header");
                *$resp.status_mut() = StatusCode::BAD_REQUEST;
                *$resp.body_mut() = Body::from("{\"detail\": \"Invalid accept header\"}");
                return;
            }
        }
    }
}

/// Main handler.
pub fn handler(req: Request<Body>) -> BoxFut {
    // Prepare response
    let mut response = Response::new(Body::empty());

    // Verify headers
    match req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()) {
        Some(uagent) if uagent.contains("Threema") => {},
        _ => {
            warn!("Received request without valid user agent");
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Box::new(future::ok(response));
        }
    }

    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => handle_index(&mut response),
        (&Method::GET, "/config") => handle_config(&req, &mut response),
        _ => handle_404(&mut response),
    }
    Box::new(future::ok(response))
}

fn handle_index(response: &mut Response<Body>) {
    *response.body_mut() = Body::from(format!("rustysafe {}", VERSION));
}

fn handle_config(request: &Request<Body>, response: &mut Response<Body>) {
    require_accept_starts_with!(request, response, "application/json");

    *response.status_mut() = StatusCode::OK;
    *response.body_mut() = Body::from(format!(
        "{{\"maxBackupBytes\": {}, \"retentionDays\": {}}}",
        SERVER_CONFIG.max_backup_bytes,
        SERVER_CONFIG.retention_days,
    ));
}

fn handle_404(response: &mut Response<Body>) {
    *response.status_mut() = StatusCode::NOT_FOUND;
}
