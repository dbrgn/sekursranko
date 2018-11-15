extern crate hyper;
extern crate rustysafe;

use hyper::Server;
use hyper::rt::Future;
use hyper::service::service_fn;

fn main() {
    let port = 3000;
    let addr = ([127, 0, 0, 1], port).into();
    println!("Starting server on port {}", port);

    // Create server
    let server = Server::bind(&addr)
        .serve(|| service_fn(rustysafe::handler))
        .map_err(|e| eprintln!("Server error: {}", e));

    // Loop forever
    hyper::rt::run(server);
}
