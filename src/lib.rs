extern crate futures;
extern crate hyper;
extern crate log;
extern crate route_recognizer;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;

mod config;

pub use config::{ServerConfig, ServerConfigPublic};

use futures::future;
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::Future;
use log::{warn, error};
use route_recognizer::{Router, Match};

static NAME: &str = "Sekurŝranko";
static VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone)]
enum Handler {
    Index,
    Config,
}

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
    let mut resp = Response::new(Body::empty());

    // Verify headers
    match req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()) {
        Some(uagent) if uagent.contains("Threema") => {},
        _ => {
            warn!("Received request without valid user agent");
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            return Box::new(future::ok(resp));
        }
    }

    // Route to handlers // TODO: Don't construct inside handler
    let mut router = Router::new();
    router.add("/", Handler::Index);
    router.add("/config", Handler::Config);

    match req.method() {
        &Method::GET => {
            match router.recognize(req.uri().path()) {
                Ok(Match { handler: Handler::Index, .. }) => handle_index(&mut resp),
                Ok(Match { handler: Handler::Config, .. }) => handle_config(&req, &mut resp, &config),
                Err(_) => handle_404(&mut resp),
            };
        }
        _ => handle_404(&mut resp),
    }

    Box::new(future::ok(resp))
}

fn handle_index(resp: &mut Response<Body>) {
    *resp.body_mut() = Body::from(format!("{} {}", NAME, VERSION));
}

fn handle_config(req: &Request<Body>, resp: &mut Response<Body>, config: &ServerConfig) {
    require_accept_starts_with!(req, resp, "application/json");
    let config_string = match serde_json::to_string(&ServerConfigPublic::from(config)) {
        Ok(s) => s,
        Err(e) => {
            error!("Could not serialize server config: {}", e);
            *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return;
        },
    };
    *resp.status_mut() = StatusCode::OK;
    *resp.body_mut() = Body::from(config_string);
}

fn handle_create_backup(req: &mut Request<Body>, resp: &mut Response<Body>) {
}

fn handle_404(resp: &mut Response<Body>) {
    *resp.status_mut() = StatusCode::NOT_FOUND;
}
