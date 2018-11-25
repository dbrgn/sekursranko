/// The server configuration.
/// TODO: Serialize
#[derive(Debug, Copy, Clone)]
pub struct ServerConfig {
    pub max_backup_bytes: u32,
    pub retention_days: u32,
}
