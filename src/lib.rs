#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::stutter)]
#![allow(clippy::non_ascii_literal)]

mod config;

use std::sync::Arc;

use futures::future;
use futures_fs::{FsPool, ReadOptions};
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::Future;
use hyper::service::{Service, NewService};
use log::{trace, warn, error};
use rand::Rng;
use route_recognizer::{Router, Match};

pub use crate::config::{ServerConfig, ServerConfigPublic};

static NAME: &str = "Sekurŝranko";
static VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Route {
    Index,
    Config,
    Backup,
}

type BoxFut = Box<dyn Future<Item=Response<Body>, Error=hyper::Error> + Send>;

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

macro_rules! require_content_type_is {
    ($req:expr, $resp:expr, $accept:expr) => {
        if $req.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) != Some($accept) {
            warn!("Received request without valid content-type header");
            *$resp.status_mut() = StatusCode::BAD_REQUEST;
            *$resp.body_mut() = Body::from("{\"detail\": \"Invalid content-type header\"}");
            return;
        }
    }
}

pub type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = hyper::Error> + Send>;

#[derive(Debug, Clone)]
pub struct BackupService {
    config: ServerConfig,
    fs_pool: Arc<FsPool>,
}

impl BackupService {
    pub fn new(config: ServerConfig) -> Self {
        let io_threads = config.io_threads;
        Self {
            config,
            fs_pool: Arc::new(FsPool::new(io_threads)),
        }
    }
}

impl Service for BackupService {
    type ReqBody = Body;
	type ResBody = Body;
	type Error = hyper::Error;
    type Future = ResponseFuture;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        trace!("BackupService::call");
        handler(&req, &self.config, &self.fs_pool)
    }
}

impl NewService for BackupService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type InitError = hyper::Error;
    type Service = Self;
    type Future = Box<dyn Future<Item = Self::Service, Error = Self::InitError> + Send>;
    fn new_service(&self) -> Self::Future {
        trace!("BackupService::new_service");
        Box::new(future::ok(self.clone()))
    }
}

/// Main handler.
pub fn handler(req: &Request<Body>, config: &ServerConfig, fs_pool: &FsPool) -> BoxFut {
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

    match router.recognize(req.uri().path()) {
        Ok(Match { handler: Route::Index, .. }) => {
            if req.method() == Method::GET {
                handle_index(&mut resp);
            } else {
                handle_405(&mut resp);
            }
        }
        Ok(Match { handler: Route::Config, .. }) => {
            if req.method() == Method::GET {
                handle_config(&req, &mut resp, &config);
            } else {
                handle_405(&mut resp);
            }
        }
        Ok(Match { handler: Route::Backup, params }) => {
            match *req.method() {
                Method::GET => handle_get_backup(
                    &req,
                    &mut resp,
                    config,
                    fs_pool,
                    params.find("backupId").expect("Missing backupId param"),
                ),
                Method::PUT => handle_put_backup(
                    &req,
                    &mut resp,
                    config,
                    fs_pool,
                    params.find("backupId").expect("Missing backupId param"),
                ),
                _ => handle_405(&mut resp),
            }
        }
        Err(_) => handle_404(&mut resp),
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
    fs_pool: &FsPool,
    backup_id: &str,
) {
    // Validate headers
    require_accept_is!(req, resp, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!("Download of backup with invalid id was requested: {}", backup_id);
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return;
    }

    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && backup_path.is_file() {
        let stream = fs_pool.read(backup_path, ReadOptions::default());
        *resp.body_mut() = Body::wrap_stream(stream);
    } else {
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return;
    }
}

fn handle_put_backup(
    req: &Request<Body>,
    resp: &mut Response<Body>,
    config: &ServerConfig,
    fs_pool: &FsPool,
    backup_id: &str,
) {
    // Validate headers
    require_content_type_is!(req, resp, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!("Upload of backup with invalid id was requested: {}", backup_id);
        *resp.status_mut() = StatusCode::BAD_REQUEST;
        *resp.body_mut() = Body::from("{\"detail\": \"Invalid backup ID\"}");
        return;
    }

    // Validate backup path
    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && !backup_path.is_file() {
        warn!("Tried to upload to a backup path that exists but is not a file: {:?}", backup_path);
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        *resp.body_mut() = Body::from("{\"detail\": \"Internal server error\"}");
        return;
    }

    // Get Content-Length header
    let content_length: u64 = match req.headers()
                                       .get(header::CONTENT_LENGTH)
                                       .and_then(|v| v.to_str().ok())
                                       .and_then(|v| v.parse().ok()) {
        Some(len) => {
            println!("content length is {}", len);
            len
        },
        None => {
            warn!("Upload request has invalid content-length header: \"{:?}\"", req.headers().get(header::CONTENT_LENGTH));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            *resp.body_mut() = Body::from("{\"detail\": \"Invalid or missing content-length header\"}");
            return;
        }
    };
    if content_length > config.max_backup_bytes {
        warn!("Upload request is too large ({} > {})", content_length, config.max_backup_bytes);
        *resp.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
        *resp.body_mut() = Body::from("{\"detail\": \"Backup is too large\"}");
        return;
    }

    // We will now write the incoming stream to a file, but in case it is too
    // large we have to abort. To prevent overwriting existing backups with
    // invalid data, first write the file to a new path and then rename it later if
    // it's OK.
    let random_ext: String = {
        let mut rng = rand::thread_rng();
        std::iter::repeat(())
            .map(|_| rng.sample(rand::distributions::Alphanumeric))
            .take(10)
            .collect()
    };
    let backup_path_dl = backup_path.with_extension(random_ext);
    trace!("Writing temporary upload to {:?}", backup_path_dl);
    if backup_path_dl.exists() {
        error!("Random upload path \"{:?}\" already exists!", backup_path_dl);
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        *resp.body_mut() = Body::from("{\"detail\": \"Internal server error\"}");
        return;
    }

//
//    // Write data
//        *resp.status_mut() = StatusCode::NOT_FOUND;
//        return;
//    }
}

fn handle_404(resp: &mut Response<Body>) {
    *resp.status_mut() = StatusCode::NOT_FOUND;
}

fn handle_405(resp: &mut Response<Body>) {
    *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
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
