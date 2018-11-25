extern crate futures;
extern crate hyper;
extern crate log;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

mod config;

pub use config::ServerConfig;

use futures::future;
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::Future;
use log::{warn, error};

static VERSION: &str = env!("CARGO_PKG_VERSION");

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
pub fn handler(req: Request<Body>, config: ServerConfig) -> BoxFut {
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
        (&Method::GET, "/config") => handle_config(&req, &mut response, &config),
        _ => handle_404(&mut response),
    }
    Box::new(future::ok(response))
}

fn handle_index(response: &mut Response<Body>) {
    *response.body_mut() = Body::from(format!("rustysafe {}", VERSION));
}

fn handle_config(request: &Request<Body>, response: &mut Response<Body>, config: &ServerConfig) {
    require_accept_starts_with!(request, response, "application/json");
    let config_string = match serde_json::to_string(config) {
        Ok(s) => s,
        Err(e) => {
            error!("Could not serialize server config: {}", e);
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return;
        },
    };
    *response.status_mut() = StatusCode::OK;
    *response.body_mut() = Body::from(config_string);
}

fn handle_404(response: &mut Response<Body>) {
    *response.status_mut() = StatusCode::NOT_FOUND;
}
