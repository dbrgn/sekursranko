use std::path::Path;

use clap::{Arg, App};
use env_logger;
use futures::future::TryFutureExt;
use hyper::Server;
use log::error;

use sekursranko::{MakeBackupService, ServerConfig};

#[tokio::main]
async fn main() -> Result<(), ()> {
    env_logger::init();

    let matches = App::new(sekursranko::NAME)
        .version(sekursranko::VERSION)
        .author("Danilo Bargen <mail@dbrgn.ch>")
        .about("An efficient and memory-safe Threema Safe server implementation written in Rust.")
        .arg(Arg::with_name("config")
             .short("c")
             .long("config")
             .value_name("FILE")
             .help("Path to the config file")
             .takes_value(true)
             .required(true))
        .get_matches();

    // Load config
    let config_path_str = matches.value_of("config").expect("Could not find config argument");
    let config_path = Path::new(config_path_str);
    let config: ServerConfig = ServerConfig::from_file(config_path).unwrap_or_else(|e| {
        eprintln!("Could not load config file: {}", e);
        ::std::process::exit(1);
    });
    let addr: ::std::net::SocketAddr = config.listen_on.parse().unwrap_or_else(|e| {
        eprintln!("Invalid listening address: {}", e);
        ::std::process::exit(1);
    });
    println!("Starting {} server with the following configuration:\n\n{}", sekursranko::NAME, &config);

    // Create server
    let context = MakeBackupService::new(config);
    let server = Server::bind(&addr)
        .serve(context)
        .map_err(|e| error!("Server error: {}", e));

    // Serve
    server.await?;

    Ok(())
}
