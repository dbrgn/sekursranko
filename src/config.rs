use std::convert::From;

/// The server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub max_backup_bytes: u32,
    pub retention_days: u32,
}

#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigPublic {
    pub max_backup_bytes: u32,
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
