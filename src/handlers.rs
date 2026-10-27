use std::{convert::Infallible, path::Path};

use anyhow::Context;
use http_body_util::{BodyExt, Full};
use hyper::{
    Method, Request, Response, StatusCode,
    body::{Bytes, Incoming},
    header,
};
use log::{debug, error, info, trace, warn};
use tokio::{fs, io::AsyncWriteExt};

use crate::{
    config::{ServerConfig, ServerConfigPublic},
    routing::{Route, recognize},
};

macro_rules! require_accept_starts_with {
    ($req:expr, $accept:expr) => {
        match $req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
        {
            Some(accept) if accept.starts_with($accept) => {}
            _ => {
                warn!("Received request without valid accept header");
                return response_400_bad_request("{\"detail\": \"Invalid accept header\"}");
            }
        }
    };
}

macro_rules! require_accept_is {
    ($req:expr, $accept:expr) => {
        if $req
            .headers()
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            != Some($accept)
        {
            warn!("Received request without valid accept header");
            return response_400_bad_request("{\"detail\": \"Invalid accept header\"}");
        }
    };
}

macro_rules! require_content_type_is {
    ($req:expr, $accept:expr) => {
        if $req
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            != Some($accept)
        {
            warn!("Received request without valid content-type header");
            return response_400_bad_request("{\"detail\": \"Invalid content-type header\"}");
        }
    };
}

/// Main handler.
pub async fn handler(
    req: Request<Incoming>,
    config: &ServerConfig,
) -> Result<Response<Full<Bytes>>, Infallible> {
    // Verify headers
    if !config.allow_browser.unwrap_or(false) {
        match req
            .headers()
            .get(header::USER_AGENT)
            .and_then(|v| v.to_str().ok())
        {
            Some(uagent) if uagent.contains("Threema") => {}
            _ => {
                warn!("Received request without valid user agent");
                return Ok(Response::builder()
                    .status(StatusCode::BAD_REQUEST)
                    .body(Full::default())
                    .expect("Could not create response"));
            }
        }
    }

    // Copy the path, so that the request body can be consumed while the
    // matched route still borrows from it
    let path = req.uri().path().to_owned();
    let mut response = if let Some(route) = recognize(&path) {
        match route {
            Route::Index => {
                if req.method() == Method::GET {
                    handle_index()
                } else {
                    response_405_method_not_allowed()
                }
            }
            Route::Config => {
                if req.method() == Method::GET {
                    handle_config(&req, config)
                } else {
                    response_405_method_not_allowed()
                }
            }
            Route::Backup { backup_id } => match *req.method() {
                Method::GET | Method::HEAD => handle_get_backup(&req, config, backup_id).await,
                Method::PUT => handle_put_backup(req, config, backup_id).await,
                Method::DELETE => handle_delete_backup(config, backup_id).await,
                _ => response_405_method_not_allowed(),
            },
        }
    } else {
        response_404_not_found()
    };

    let headers = response.headers_mut();
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        header::HeaderValue::from_static("nosniff"),
    );
    if config.allow_browser.unwrap_or(false) {
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::HeaderValue::from_static("*"),
        );
    }

    Ok(response)
}

fn handle_index() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Full::from(format!("{} {}", crate::NAME, crate::VERSION)))
        .expect("Could not create response")
}

fn handle_config(req: &Request<Incoming>, config: &ServerConfig) -> Response<Full<Bytes>> {
    require_accept_starts_with!(req, "application/json");
    let config_string = match serde_json::to_string(&ServerConfigPublic::from(config)) {
        Ok(s) => s,
        Err(e) => {
            error!("Could not serialize server config: {}", e);
            return response_500_internal_server_error();
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::from(config_string))
        .expect("Could not create response")
}

/// Shorten a backup id for logging.
///
/// Anyone who knows a backup id can download, overwrite or delete that
/// backup, so full ids must not end up in log files.
fn log_id(backup_id: &str) -> String {
    let prefix: String = backup_id.chars().take(8).collect();
    format!("{}…", prefix)
}

/// Return whether this backup id is valid.
///
/// A backup id must be a 64 character lowercase hex string.
fn backup_id_valid(backup_id: &str) -> bool {
    backup_id.len() == 64
        && backup_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && (c.is_ascii_digit() || c.is_lowercase()))
}

async fn handle_get_backup(
    req: &Request<Incoming>,
    config: &ServerConfig,
    backup_id: &str,
) -> Response<Full<Bytes>> {
    // Validate headers
    require_accept_is!(req, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Download of backup with invalid id was requested: {}",
            log_id(backup_id)
        );
        return response_400_bad_request("{\"detail\": \"Invalid backup ID\"}");
    }

    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && backup_path.is_file() {
        let builder = Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/octet-stream");
        let response = if req.method() == Method::HEAD {
            // A HEAD response has no body, so the content length of the
            // backup must be set explicitly
            let length = match fs::metadata(&backup_path).await {
                Ok(metadata) => metadata.len(),
                Err(e) => {
                    error!("Could not read file metadata: {}", e);
                    return response_500_internal_server_error();
                }
            };
            builder
                .header(header::CONTENT_LENGTH, length)
                .body(Full::default())
        } else {
            let bytes = match fs::read(backup_path).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Could not read file: {}", e);
                    return response_500_internal_server_error();
                }
            };
            builder.body(Full::from(bytes))
        };
        response.expect("Could not create response")
    } else {
        response_404_not_found()
    }
}

async fn handle_put_backup(
    req: Request<Incoming>,
    config: &ServerConfig,
    backup_id: &str,
) -> Response<Full<Bytes>> {
    // Validate headers
    require_content_type_is!(req, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Upload of backup with invalid id was requested: {}",
            log_id(backup_id)
        );
        return response_400_bad_request("{\"detail\": \"Invalid backup ID\"}");
    }

    // Validate backup path
    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && !backup_path.is_file() {
        warn!(
            "Tried to upload backup {}, but its path exists and is not a file",
            log_id(backup_id)
        );
        return response_500_internal_server_error();
    }

    // Get Content-Length header
    // We can trust that the actual body size will not be larger than the
    // declared content length, because hyper will actually stop consuming data
    // after the declared number of bytes have been processed.
    let content_length: Option<u64> = req
        .headers()
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    if let Some(length) = content_length {
        if length > config.max_backup_bytes {
            warn!(
                "Upload request is too large ({} > {})",
                length, config.max_backup_bytes
            );
            return Response::builder()
                .status(StatusCode::PAYLOAD_TOO_LARGE)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Full::from("{\"detail\": \"Backup is too large\"}"))
                .expect("Could not create response");
        }
    } else {
        warn!(
            "Upload request has invalid content-length header: \"{:?}\"",
            req.headers().get(header::CONTENT_LENGTH)
        );
        return response_400_bad_request(
            "{\"detail\": \"Invalid or missing content-length header\"}",
        );
    };

    // Write backup
    match write_backup(req.into_body(), backup_id, &backup_path).await {
        Ok(updated) => {
            info!(
                "{} backup {}",
                if updated { "Updated" } else { "Created" },
                log_id(backup_id)
            );
            Response::builder()
                .status(if updated {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::CREATED
                })
                .body(Full::default())
                .expect("Could not create response")
        }
        Err(e) => {
            error!("Could not write backup: {}", e);
            response_500_internal_server_error()
        }
    }
}

/// Store the backup to the file system.
///
/// Return true if an existing backup was updated, or false if a new backup was created.
async fn write_backup(
    mut body: Incoming,
    backup_id: &str,
    backup_path: &Path,
) -> anyhow::Result<bool> {
    // The incoming stream will be written to a temporary file in the backup
    // directory, which is then atomically moved to the final location. This is
    // done to prevent incomplete backups from being persisted. The temporary
    // file is created exclusively with 0600 permissions, and it is deleted
    // when dropped, i.e. also when the upload fails or is aborted by the
    // client.
    let backup_dir = backup_path
        .parent()
        .context("Backup path has no parent directory")?;
    let backup_file_tmp = tempfile::Builder::new()
        .prefix(&format!("{}.", backup_id))
        .tempfile_in(backup_dir)
        .context("Could not create temporary file")?;
    trace!("Writing temporary upload for backup {}", log_id(backup_id));

    // Write data to temporary file
    let mut file = fs::File::from_std(
        backup_file_tmp
            .as_file()
            .try_clone()
            .context("Could not clone temporary file handle")?,
    );
    while let Some(frame_or_error) = body.frame().await {
        let frame = frame_or_error.context("Could not read body chunk")?;
        if let Ok(chunk) = frame.into_data() {
            file.write_all(&chunk)
                .await
                .context("Could not write chunk to temporary file")?
        }
    }

    // Wait for all pending writes to complete before moving the file
    file.flush()
        .await
        .context("Could not flush temporary file")?;
    trace!("Wrote temp backup for {}", log_id(backup_id));

    // Move temporary file to final location
    let updated = backup_path.exists() && backup_path.is_file();
    backup_file_tmp
        .persist(backup_path)
        .context("Could not move temporary backup to final location")?;
    trace!(
        "Moved temporary backup {} to final location",
        log_id(backup_id)
    );

    Ok(updated)
}

async fn handle_delete_backup(config: &ServerConfig, backup_id: &str) -> Response<Full<Bytes>> {
    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Deletion of backup with invalid id was requested: {}",
            log_id(backup_id)
        );
        return response_400_bad_request("{\"detail\": \"Invalid backup ID\"}");
    }

    let backup_path = config.backup_dir.join(backup_id);

    // Ensure backup exists
    if !backup_path.exists() {
        debug!(
            "Tried to delete backup {}, but it does not exist",
            log_id(backup_id)
        );
        return response_404_not_found();
    }

    // Ensure backup is a file
    if !backup_path.is_file() {
        warn!(
            "Tried to delete backup {}, but its path exists and is not a file",
            log_id(backup_id)
        );
        return response_500_internal_server_error();
    }

    // Delete file
    match fs::remove_file(&backup_path).await {
        Ok(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Full::default())
            .expect("Could not create response"),
        Err(e) => {
            error!("Could not delete backup {}: {}", log_id(backup_id), e);
            response_500_internal_server_error()
        }
    }
}

fn response_400_bad_request(body: &'static str) -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::from(body))
        .expect("Could not create response")
}

fn response_404_not_found() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Full::default())
        .expect("Could not create response")
}

fn response_405_method_not_allowed() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Full::default())
        .expect("Could not create response")
}

fn response_500_internal_server_error() -> Response<Full<Bytes>> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Full::from("{\"detail\": \"Internal server error\"}"))
        .expect("Could not create response")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_backup_id_valid() {
        assert!(!backup_id_valid(""));
        assert!(!backup_id_valid("0123"));
        assert!(!backup_id_valid(
            "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"
        ));

        assert!(backup_id_valid(
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn test_log_id() {
        assert_eq!(
            log_id("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"),
            "01234567…"
        );
        assert_eq!(log_id("abc"), "abc…");
        assert_eq!(
            log_id("\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}"),
            "\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}\u{e4}…"
        );
    }
}
