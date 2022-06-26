use std::path::PathBuf;

use clap::{self, Parser};
use hyper::Server;
use log::error;

use sekursranko::{MakeBackupService, ServerConfig};

#[derive(Parser, Debug)]
#[clap(author, version, about)]
struct Args {
    /// Path to the config file
    #[clap(short, long)]
    config: PathBuf,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    env_logger::init();

    // Parse CLI args
    let args = Args::parse();

    // Load config
    let config: ServerConfig = ServerConfig::from_file(&args.config).unwrap_or_else(|e| {
        eprintln!("Could not load config file: {}", e);
        ::std::process::exit(1);
    });
    let addr: ::std::net::SocketAddr = config.listen_on.parse().unwrap_or_else(|e| {
        eprintln!("Invalid listening address: {}", e);
        ::std::process::exit(1);
    });
    println!(
        "Starting {} server with the following configuration:\n\n{}",
        sekursranko::NAME,
        &config
    );

    // Create server
    let service = MakeBackupService::new(config);
    let server = Server::bind(&addr).serve(service);

    // Serve
    if let Err(e) = server.await {
        error!("Server error: {}", e);
        std::process::exit(1);
    };
}
