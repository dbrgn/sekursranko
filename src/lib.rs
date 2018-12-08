extern crate futures;
extern crate hyper;
extern crate log;
extern crate route_recognizer;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate tokio;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Route {
    Index,
    Config,
    Backup,
}

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

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

macro_rules! require_accept_is {
    ($req:expr, $resp:expr, $accept:expr) => {
        if $req.headers().get(header::ACCEPT).and_then(|v| v.to_str().ok()) != Some($accept) {
            warn!("Received request without valid accept header");
            *$resp.status_mut() = StatusCode::BAD_REQUEST;
            *$resp.body_mut() = Body::from("{\"detail\": \"Invalid accept header\"}");
            return;
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
    router.add("/", Route::Index);
    router.add("/config", Route::Config);
    router.add("/backups/:backupId", Route::Backup);

    match req.method() {
        &Method::GET => {
            match router.recognize(req.uri().path()) {
                Ok(Match { handler: Route::Index, .. }) =>
                    handle_index(&mut resp),
                Ok(Match { handler: Route::Config, .. }) =>
                    handle_config(&req, &mut resp, &config),
                Ok(Match { handler: Route::Backup, params }) =>
                    handle_get_backup(&req, &mut resp, &config,
                                      params.find("backupId")
                                            .expect("Could not get backupId param")),
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

/// Return whether this backup id is valid.
///
/// A backup id must be a 64 character lowercase hex string.
fn backup_id_valid(backup_id: &str) -> bool {
    backup_id.len() == 64 &&
    backup_id.chars().all(|c| c.is_ascii_hexdigit() && (c.is_digit(10) || c.is_lowercase()) )
}

fn handle_get_backup(
    req: &Request<Body>,
    resp: &mut Response<Body>,
    config: &ServerConfig,
    backup_id: &str,
) {
    println!("xxx");

    // Validate headers
    require_accept_is!(req, resp, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!("Download of backup with invalid id was requested: {}", backup_id);
        println!("yyy");
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return;
    }

    let backup_path = config.backup_dir.join(backup_id);
    println!("Backup path: {:?}", backup_path);
    if backup_path.exists() && backup_path.is_file() {
        let chunks = vec![
            "hello",
            " ",
            "world",
        ];
        let stream = futures::stream::iter_ok::<_, ::std::io::Error>(chunks);
        *resp.body_mut() = Body::wrap_stream(stream);
//            tokio::fs::File::open(backup_path)
//        );
    } else {
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return;
    }
}

fn handle_404(resp: &mut Response<Body>) {
    *resp.status_mut() = StatusCode::NOT_FOUND;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_backup_id_valid() {
        assert!(!backup_id_valid(""));
        assert!(!backup_id_valid("0123"));
        assert!(!backup_id_valid("gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"));

        assert!(backup_id_valid("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"));
    }
}
