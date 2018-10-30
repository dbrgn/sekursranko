extern crate futures;
extern crate hyper;

use futures::future;
use hyper::{Body, Request, Response, Server};
use hyper::{Method, StatusCode};
use hyper::rt::Future;
use hyper::service::service_fn;

static VERSION: &str = env!("CARGO_PKG_VERSION");

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

fn router(req: Request<Body>) -> BoxFut {
    let mut response = Response::new(Body::empty());
    match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => handle_index(&mut response),
        _ => handle_404(&mut response),
    }
    Box::new(future::ok(response))
}

fn handle_index(response: &mut Response<Body>) {
    *response.body_mut() = Body::from(format!("rustysafe {}", VERSION));
}

fn handle_404(response: &mut Response<Body>) {
    *response.status_mut() = StatusCode::NOT_FOUND;
}

fn main() {
    let port = 3000;
    let addr = ([127, 0, 0, 1], port).into();
    println!("Starting server on port {}", port);

    // Create server
    let server = Server::bind(&addr)
        .serve(|| service_fn(router))
        .map_err(|e| eprintln!("Server error: {}", e));

    // Loop forever
    hyper::rt::run(server);
}
