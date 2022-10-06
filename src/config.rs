use std::convert::From;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde_derive::{Deserialize, Serialize};

/// The server configuration.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ServerConfig {
    /// The max file size for backups (e.g. 65536)
    pub max_backup_bytes: u64,
    /// The number of days a backup will be retained (e.g. 180)
    pub retention_days: u32,
    /// The path to the directory where backups will be stored
    pub backup_dir: PathBuf,
    /// The listening address for the server (e.g. "127.0.0.1:3000")
    pub listen_on: String,
    /// Whether to allow access from a web browser
    ///
    /// This will disable the user-agent check and set a CORS header on the
    /// response.
    pub allow_browser: Option<bool>,
}

impl ServerConfig {
    pub fn from_file(config_path: &Path) -> Result<Self, String> {
        // Read config file
        if !config_path.exists() {
            return Err(format!("Config file at {:?} does not exist", config_path));
        }
        if !config_path.is_file() {
            return Err(format!("Config file at {:?} is not a file", config_path));
        }
        let mut file =
            File::open(config_path).map_err(|e| format!("Could not open config file: {}", e))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| format!("Could not read config file: {}", e))?;

        // Deserialize
        toml::from_str(&contents).map_err(|e| format!("Could not deserialize config file: {}", e))
    }
}

impl fmt::Display for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "- Max backup bytes: {}", self.max_backup_bytes)?;
        writeln!(f, "- Retention days: {}", self.retention_days)?;
        writeln!(f, "- Backup directory: {:?}", self.backup_dir)?;
        writeln!(f, "- Listening address: {}", self.listen_on)?;
        writeln!(
            f,
            "- Allow browser access: {}",
            self.allow_browser.unwrap_or(false)
        )?;
        Ok(())
    }
}

/// The public part of the server configuration.
///
/// This can be queried over the API.
#[derive(Debug, Copy, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfigPublic {
    /// The max file size for backups (e.g. 65536)
    pub max_backup_bytes: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::io::Write;

    use tempfile::NamedTempFile;

    #[test]
    fn read_config_file_invalid() {
        let path = Path::new("/tmp/asdfklasdfjaklsdfjlk");
        let res = ServerConfig::from_file(path);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            format!("Config file at {:?} does not exist", path)
        );
    }

    #[test]
    fn read_config_file_no_file() {
        let path = Path::new("/bin");
        let res = ServerConfig::from_file(path);
        assert!(res.is_err());
        assert_eq!(
            res.unwrap_err(),
            format!("Config file at {:?} is not a file", path)
        );
    }

    #[test]
    fn read_config_file_ok() {
        let mut tempfile = NamedTempFile::new().unwrap();
        let file = tempfile.as_file_mut();
        file.write_all(b"max_backup_bytes = 10000\n").unwrap();
        file.write_all(b"retention_days = 100\n").unwrap();
        file.write_all(b"backup_dir = \"backups\"\n").unwrap();
        file.write_all(b"listen_on = \"127.0.0.1:3000\"\n").unwrap();
        file.write_all(b"allow_browser = true\n").unwrap();
        let res = ServerConfig::from_file(tempfile.path());
        assert_eq!(
            res.unwrap(),
            ServerConfig {
                max_backup_bytes: 10_000,
                retention_days: 100,
                backup_dir: PathBuf::from("backups"),
                listen_on: "127.0.0.1:3000".to_string(),
                allow_browser: Some(true),
            }
        );
    }
}
