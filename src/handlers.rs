use std::fs;
use std::io::{Error as IoError, ErrorKind};

use futures::{future, sink::Sink};
use futures_fs::{FsPool, ReadOptions, WriteOptions};
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::{Future, Stream};
use log::{trace, info, warn, error};
use rand::Rng;
use route_recognizer::{Router, Match};

use crate::config::{ServerConfig, ServerConfigPublic};

type BoxFut = Box<dyn Future<Item=Response<Body>, Error=hyper::Error> + Send>;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Route {
    Index,
    Config,
    Backup,
}

macro_rules! require_accept_starts_with {
    ($req:expr, $resp:expr, $accept:expr) => {
        match $req.headers().get(header::ACCEPT).and_then(|v| v.to_str().ok()) {
            Some(accept) if accept.starts_with($accept) => {},
            _ => {
                warn!("Received request without valid accept header");
                *$resp.status_mut() = StatusCode::BAD_REQUEST;
                *$resp.body_mut() = Body::from("{\"detail\": \"Invalid accept header\"}");
                return Box::new(future::ok($resp));
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
            return Box::new(future::ok($resp));
        }
    }
}

macro_rules! require_content_type_is {
    ($req:expr, $resp:expr, $accept:expr) => {
        if $req.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) != Some($accept) {
            warn!("Received request without valid content-type header");
            *$resp.status_mut() = StatusCode::BAD_REQUEST;
            *$resp.body_mut() = Body::from("{\"detail\": \"Invalid content-type header\"}");
            return Box::new(future::ok($resp));
        }
    }
}

/// Main handler.
pub fn handler(req: Request<Body>, config: &ServerConfig, fs_pool: &FsPool) -> BoxFut {
    // Verify headers
    match req.headers().get(header::USER_AGENT).and_then(|v| v.to_str().ok()) {
        Some(uagent) if uagent.contains("Threema") => {},
        _ => {
            warn!("Received request without valid user agent");
            let mut resp = Response::new(Body::empty());
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
                handle_index()
            } else {
                handle_405()
            }
        }
        Ok(Match { handler: Route::Config, .. }) => {
            if req.method() == Method::GET {
                handle_config(&req, &config)
            } else {
                handle_405()
            }
        }
        Ok(Match { handler: Route::Backup, params }) => {
            match *req.method() {
                Method::GET => handle_get_backup(
                    &req,
                    config,
                    fs_pool,
                    params.find("backupId").expect("Missing backupId param"),
                ),
                Method::PUT => handle_put_backup(
                    req,
                    config,
                    fs_pool,
                    params.find("backupId").expect("Missing backupId param"),
                ),
                _ => handle_405(),
            }
        }
        Err(_) => handle_404(),
    }
}

fn handle_index() -> BoxFut {
    let mut resp = Response::new(Body::empty());
    *resp.body_mut() = Body::from(format!("{} {}", crate::NAME, crate::VERSION));
    Box::new(future::ok(resp))
}

fn handle_config(req: &Request<Body>, config: &ServerConfig) -> BoxFut {
    let mut resp = Response::new(Body::empty());
    require_accept_starts_with!(req, resp, "application/json");
    let config_string = match serde_json::to_string(&ServerConfigPublic::from(config)) {
        Ok(s) => s,
        Err(e) => {
            error!("Could not serialize server config: {}", e);
            *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Box::new(future::ok(resp));
        },
    };
    *resp.status_mut() = StatusCode::OK;
    *resp.body_mut() = Body::from(config_string);
    Box::new(future::ok(resp))
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
    config: &ServerConfig,
    fs_pool: &FsPool,
    backup_id: &str,
) -> BoxFut {
    let mut resp = Response::new(Body::empty());

    // Validate headers
    require_accept_is!(req, resp, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!("Download of backup with invalid id was requested: {}", backup_id);
        *resp.status_mut() = StatusCode::NOT_FOUND;
        return Box::new(future::ok(resp));
    }

    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && backup_path.is_file() {
        let stream = fs_pool.read(backup_path, ReadOptions::default());
        *resp.body_mut() = Body::wrap_stream(stream);
    } else {
        *resp.status_mut() = StatusCode::NOT_FOUND;
    }

    Box::new(future::ok(resp))
}

fn handle_put_backup(
    req: Request<Body>,
    config: &ServerConfig,
    fs_pool: &FsPool,
    backup_id: &str,
) -> BoxFut {
    // Prepare response
    let mut resp = Response::new(Body::empty());

    // Validate headers
    require_content_type_is!(req, resp, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!("Upload of backup with invalid id was requested: {}", backup_id);
        *resp.status_mut() = StatusCode::BAD_REQUEST;
        *resp.body_mut() = Body::from("{\"detail\": \"Invalid backup ID\"}");
        return Box::new(future::ok(resp));
    }

    // Validate backup path
    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && !backup_path.is_file() {
        warn!("Tried to upload to a backup path that exists but is not a file: {:?}", backup_path);
        *resp.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        *resp.body_mut() = Body::from("{\"detail\": \"Internal server error\"}");
        return Box::new(future::ok(resp));
    }

    // Get Content-Length header
    let content_length: u64 = match req.headers()
                                       .get(header::CONTENT_LENGTH)
                                       .and_then(|v| v.to_str().ok())
                                       .and_then(|v| v.parse().ok()) {
        Some(len) => len,
        None => {
            warn!("Upload request has invalid content-length header: \"{:?}\"", req.headers().get(header::CONTENT_LENGTH));
            *resp.status_mut() = StatusCode::BAD_REQUEST;
            *resp.body_mut() = Body::from("{\"detail\": \"Invalid or missing content-length header\"}");
            return Box::new(future::ok(resp));
        }
    };
    if content_length > config.max_backup_bytes {
        warn!("Upload request is too large ({} > {})", content_length, config.max_backup_bytes);
        *resp.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
        *resp.body_mut() = Body::from("{\"detail\": \"Backup is too large\"}");
        return Box::new(future::ok(resp));
    }

    // Write the incoming stream to a temporary file. This is done to prevent
    // incomplete backups from being persisted.
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
        return Box::new(future::ok(resp));
    }

    // Write data
    let backup_id = backup_id.to_string();
    let sink = fs_pool.write(backup_path_dl.clone(), WriteOptions::default());
    let body_stream = req
        .into_body()
        .map(|chunk| chunk.into_bytes())
        .map_err(|error: hyper::Error| IoError::new(ErrorKind::Other, error));
    let write_future = sink.send_all(body_stream);
    let response_future = write_future
        .and_then(move |_| {
            trace!("Wrote temp backup for {}", backup_id);
            let updated = backup_path.exists() && backup_path.is_file();
            match fs::rename(&backup_path_dl, &backup_path) {
                Ok(_) => trace!("Renamed: {:?} -> {:?}", backup_path_dl, backup_path),
                Err(e) => {
                    error!("Could not rename backup: {}", e);
                    return Err(e);
                },
            }
            info!("Wrote backup {}", backup_id);
            Ok(Response::builder()
               .status(if updated { StatusCode::NO_CONTENT } else { StatusCode::CREATED })
               .body(Body::empty())
               .expect("Could not create response"))
        })
        .or_else(move |err: IoError| {
            error!("Could not write backup: {}", err);
            Ok(Response::builder()
               .status(StatusCode::INTERNAL_SERVER_ERROR)
               .body(Body::empty())
               .expect("Could not create response"))
        });
    Box::new(response_future)
}

fn handle_404() -> BoxFut {
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = StatusCode::NOT_FOUND;
    Box::new(future::ok(resp))
}

fn handle_405() -> BoxFut {
    let mut resp = Response::new(Body::empty());
    *resp.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
    Box::new(future::ok(resp))
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
