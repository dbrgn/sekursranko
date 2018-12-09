use std::convert::From;
use std::path::PathBuf;

use serde_derive::Serialize;

/// The server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// The max file size for backups (e.g. 65536)
    pub max_backup_bytes: u32,
    /// The number of days a backup will be retained (e.g. 180)
    pub retention_days: u32,
    /// The path to the directory where backups will be stored
    pub backup_dir: PathBuf,
    /// The number of threads for doing I/O (e.g. 4)
    pub io_threads: usize,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigPublic {
    /// The max file size for backups (e.g. 65536)
    pub max_backup_bytes: u32,
    /// The number of days a backup will be retained (e.g. 180)
    pub retention_days: u32,
}

impl<'a> From<&'a ServerConfig> for ServerConfigPublic {
    fn from(other: &'a ServerConfig) -> Self {
        Self {
            max_backup_bytes: other.max_backup_bytes,
            retention_days: other.retention_days,
        }
    }
}
