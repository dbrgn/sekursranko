extern crate futures;
extern crate hyper;
extern crate log;

use futures::future;
use hyper::{Body, Request, Response};
use hyper::{Method, StatusCode};
use hyper::header;
use hyper::rt::Future;
use log::warn;

static VERSION: &str = env!("CARGO_PKG_VERSION");

type BoxFut = Box<Future<Item=Response<Body>, Error=hyper::Error> + Send>;

/// Main handler.
pub fn handler(req: Request<Body>) -> BoxFut {
    // Prepare response
    let mut response = Response::new(Body::empty());

    // Verify headers
    match req.headers().get(header::ACCEPT).and_then(|v| v.to_str().ok()) {
        Some(accept) if accept.starts_with("application/json") => {},
        _ => {
            warn!("Received request without accept header");
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Box::new(future::ok(response));
        }
    }

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

