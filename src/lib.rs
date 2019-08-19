#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::non_ascii_literal)]

mod config;
mod handlers;
mod service;

pub use crate::config::{ServerConfig, ServerConfigPublic};
pub use crate::service::BackupService;

pub static NAME: &str = "Sekurŝranko";
pub static VERSION: &str = env!("CARGO_PKG_VERSION");
