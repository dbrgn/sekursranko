#![deny(clippy::all)]

mod config;
mod handlers;
mod routing;
mod service;

pub use crate::{
    config::{ServerConfig, ServerConfigPublic},
    service::{BackupService, MakeBackupService},
};

pub static NAME: &str = "Sekurŝranko";
pub static VERSION: &str = env!("CARGO_PKG_VERSION");
