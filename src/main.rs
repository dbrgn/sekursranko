extern crate hyper;
extern crate sekursranko;

use hyper::Server;
use hyper::rt::Future;
use hyper::service::service_fn;

use sekursranko::ServerConfig;

fn main() {
    let port = 3000;
    let addr = ([127, 0, 0, 1], port).into();
    println!("Starting server on port {}", port);

    // Create server
    let config: ServerConfig = ServerConfig {
        max_backup_bytes: 524288,
        retention_days: 180,
    };
    let server = Server::bind(&addr)
        .serve(move || {
            let config_clone = config.clone();
            service_fn(move |req| sekursranko::handler(req, config_clone))
        })
        .map_err(|e| eprintln!("Server error: {}", e));

    // Loop forever
    hyper::rt::run(server);
}
