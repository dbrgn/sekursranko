extern crate hyper;
extern crate reqwest;
extern crate rustysafe;

use std::boxed::Box;
use std::net::TcpListener;
use std::thread;

use hyper::Server;
use hyper::rt::Future;
use hyper::service::service_fn;

struct TestServer {
    server: Box<Future<Item = (), Error = ()> + Send + 'static>,
}

impl TestServer {
    /// Create a new test server instance (but don't start it yet).
    fn new() -> (Self, u16) {
        let port = {
            let tmp_socket = TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = tmp_socket.local_addr().unwrap();
            addr.port()
        };
        let addr = ([127, 0, 0, 1], port).into();
        let server = Server::bind(&addr)
            .serve(|| service_fn(rustysafe::handler))
            .map_err(|e| eprintln!("Server error: {}", e));
        (Self { server: Box::new(server) }, port)
    }

    fn run(self) {
        thread::spawn(move || {
            hyper::rt::run(self.server);
        });
    }
}

#[test]
fn testing_works() {
    let (srv, _) = TestServer::new();
    srv.run();
    assert_eq!(1, 1);
}

#[test]
fn get_index() {
    let (srv, port) = TestServer::new();
    srv.run();
    let res = reqwest::get(&format!("http://127.0.0.1:{}/", port));
    println!("{:?}", res);
}
