use std::{io::Error as IoError, os::unix::fs::PermissionsExt, path::Path};

use anyhow::{bail, Context};
use futures::StreamExt;
use hyper::{header, Body, Method, Request, Response, StatusCode};
use log::{debug, error, info, trace, warn};
use rand::Rng;
use tokio::{fs, io::AsyncWriteExt};

use crate::{
    config::{ServerConfig, ServerConfigPublic},
    routing::{Route, Router},
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
    req: Request<Body>,
    router: &Router,
    config: &ServerConfig,
) -> Result<Response<Body>, hyper::Error> {
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
                    .body(Body::empty())
                    .expect("Could not create response"));
            }
        }
    }

    let mut response = if let Ok(route_match) = router.recognize(req.uri().path()) {
        match route_match.handler() {
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
            Route::Backup => match *req.method() {
                Method::GET | Method::HEAD => {
                    handle_get_backup(
                        &req,
                        config,
                        route_match
                            .params()
                            .find("backupId")
                            .expect("Missing backupId param"),
                    )
                    .await
                }
                Method::PUT => {
                    handle_put_backup(
                        req,
                        config,
                        route_match
                            .params()
                            .find("backupId")
                            .expect("Missing backupId param"),
                    )
                    .await
                }
                Method::DELETE => {
                    handle_delete_backup(
                        config,
                        route_match
                            .params()
                            .find("backupId")
                            .expect("Missing backupId param"),
                    )
                    .await
                }
                _ => response_405_method_not_allowed(),
            },
        }
    } else {
        response_404_not_found()
    };

    if config.allow_browser.unwrap_or(false) {
        let headers = response.headers_mut();
        headers.insert(
            header::ACCESS_CONTROL_ALLOW_ORIGIN,
            header::HeaderValue::from_static("*"),
        );
    }

    Ok(response)
}

fn handle_index() -> Response<Body> {
    Response::builder()
        .status(StatusCode::OK)
        .body(Body::from(format!("{} {}", crate::NAME, crate::VERSION)))
        .expect("Could not create response")
}

fn handle_config(req: &Request<Body>, config: &ServerConfig) -> Response<Body> {
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
        .body(Body::from(config_string))
        .expect("Could not create response")
}

/// Return whether this backup id is valid.
///
/// A backup id must be a 64 character lowercase hex string.
fn backup_id_valid(backup_id: &str) -> bool {
    backup_id.len() == 64
        && backup_id
            .chars()
            .all(|c| c.is_ascii_hexdigit() && (c.is_digit(10) || c.is_lowercase()))
}

async fn handle_get_backup(
    req: &Request<Body>,
    config: &ServerConfig,
    backup_id: &str,
) -> Response<Body> {
    // Validate headers
    require_accept_is!(req, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Download of backup with invalid id was requested: {}",
            backup_id
        );
        return response_404_not_found();
    }

    let is_head_request = req.method() == Method::HEAD;

    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && backup_path.is_file() {
        let body: Body = if is_head_request {
            Body::empty()
        } else {
            let bytes = match fs::read(backup_path).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Could not read file: {}", e);
                    return response_500_internal_server_error();
                }
            };
            bytes.into()
        };
        Response::builder()
            .status(StatusCode::OK)
            .body(body)
            .expect("Could not create response")
    } else {
        response_404_not_found()
    }
}

// Create a file with permissions set to 0600.
async fn create_file(path: &Path) -> Result<fs::File, IoError> {
    let file = fs::File::create(path).await?;
    let mut perms = file.metadata().await?.permissions();
    perms.set_mode(0o600);
    file.set_permissions(perms).await?;
    Ok(file)
}

async fn handle_put_backup(
    req: Request<Body>,
    config: &ServerConfig,
    backup_id: &str,
) -> Response<Body> {
    // Validate headers
    require_content_type_is!(req, "application/octet-stream");

    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Upload of backup with invalid id was requested: {}",
            backup_id
        );
        return response_400_bad_request("{\"detail\": \"Invalid backup ID\"}");
    }

    // Validate backup path
    let backup_path = config.backup_dir.join(backup_id);
    if backup_path.exists() && !backup_path.is_file() {
        warn!(
            "Tried to upload to a backup path that exists but is not a file: {:?}",
            backup_path
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
                .body(Body::from("{\"detail\": \"Backup is too large\"}"))
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
                backup_id
            );
            Response::builder()
                .status(if updated {
                    StatusCode::NO_CONTENT
                } else {
                    StatusCode::CREATED
                })
                .body(Body::empty())
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
async fn write_backup(mut body: Body, backup_id: &str, backup_path: &Path) -> anyhow::Result<bool> {
    // The incoming stream will be written to a temporary file. This is done to prevent
    // incomplete backups from being persisted.
    let random_ext: String = {
        let mut rng = rand::thread_rng();
        std::iter::repeat(())
            .map(|_| rng.sample(rand::distributions::Alphanumeric))
            .map(char::from)
            .take(10)
            .collect()
    };
    let backup_path_dl = backup_path.with_extension(random_ext);
    trace!("Writing temporary upload to {:?}", backup_path_dl);
    if backup_path_dl.exists() {
        bail!(
            "Random upload path \"{:?}\" already exists!",
            backup_path_dl
        );
    }

    // Create the empty download file to ensure correct permissions before
    // writing the data
    let mut backup_file_dl = create_file(&backup_path_dl)
        .await
        .context("Could not create temporary file")?;

    // Write data to temporary file
    while let Some(chunk_or_error) = body.next().await {
        let chunk = chunk_or_error.context("Could not read body chunk")?;
        backup_file_dl
            .write_all(&chunk)
            .await
            .context("Could not write chunk to temporary file")?
    }
    trace!("Wrote temp backup for {}", backup_id);

    // Move temporary file to final location
    let updated = backup_path.exists() && backup_path.is_file();
    fs::rename(&backup_path_dl, &backup_path)
        .await
        .context("Could not move temporary backup to final location")?;
    trace!("Renamed: {:?} -> {:?}", backup_path_dl, backup_path);

    Ok(updated)
}

async fn handle_delete_backup(config: &ServerConfig, backup_id: &str) -> Response<Body> {
    // Validate params
    if !backup_id_valid(backup_id) {
        warn!(
            "Deletion of backup with invalid id was requested: {}",
            backup_id
        );
        return response_400_bad_request("{\"detail\": \"Invalid backup ID\"}");
    }

    let backup_path = config.backup_dir.join(backup_id);

    // Ensure backup exists
    if !backup_path.exists() {
        debug!(
            "Tried to delete a backup path that does not exist: {:?}",
            backup_path
        );
        return response_404_not_found();
    }

    // Ensure backup is a file
    if !backup_path.is_file() {
        warn!(
            "Tried to delete a backup path that exists but is not a file: {:?}",
            backup_path
        );
        return response_500_internal_server_error();
    }

    // Delete file
    match fs::remove_file(&backup_path).await {
        Ok(_) => Response::builder()
            .status(StatusCode::NO_CONTENT)
            .body(Body::empty())
            .expect("Could not create response"),
        Err(e) => {
            error!("Could not delete backup at {:?}: {}", &backup_path, e);
            response_500_internal_server_error()
        }
    }
}

fn response_400_bad_request(body: &'static str) -> Response<Body> {
    Response::builder()
        .status(StatusCode::BAD_REQUEST)
        .body(Body::from(body))
        .expect("Could not create response")
}

fn response_404_not_found() -> Response<Body> {
    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::empty())
        .expect("Could not create response")
}

fn response_405_method_not_allowed() -> Response<Body> {
    Response::builder()
        .status(StatusCode::METHOD_NOT_ALLOWED)
        .body(Body::empty())
        .expect("Could not create response")
}

fn response_500_internal_server_error() -> Response<Body> {
    Response::builder()
        .status(StatusCode::INTERNAL_SERVER_ERROR)
        .body(Body::from("{\"detail\": \"Internal server error\"}"))
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
}
