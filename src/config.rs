/// The server configuration.
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    pub max_backup_bytes: u32,
    pub retention_days: u32,
}
