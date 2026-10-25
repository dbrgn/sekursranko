use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc, time::Duration};

use http_body_util::Full;
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1,
    service::Service,
    Request, Response,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{debug, error, trace};
use tokio::net::TcpListener;

use crate::{
    config::ServerConfig,
    handlers::handler,
    routing::{make_router, Router},
};

/// A `BackupService` wraps a configuration and a router.
#[derive(Debug, Clone)]
pub struct BackupService {
    config: Arc<ServerConfig>,
    router: Arc<Router>,
}

impl BackupService {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config: Arc::new(config),
            router: Arc::new(make_router()),
        }
    }
}

type PinBox<T> = Pin<Box<T>>;

impl Service<Request<Incoming>> for BackupService {
    type Response = Response<Full<Bytes>>;
    type Error = Infallible;
    type Future = PinBox<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>;

    fn call(&self, req: Request<Incoming>) -> Self::Future {
        trace!("BackupService::call");

        // Copy Arc references that will be moved into the future
        let config = self.config.clone();
        let router = self.router.clone();

        // Call handler
        Box::pin(async move { handler(req, &router, &config).await })
    }
}

/// Accept connections on the listener and serve them with a `BackupService`.
///
/// This function only returns if the server cannot continue serving.
pub async fn serve(listener: TcpListener, config: ServerConfig) {
    let service = BackupService::new(config);
    loop {
        let (stream, remote_addr) = match listener.accept().await {
            Ok(conn) => conn,
            Err(e) => {
                // Accept errors (e.g. too many open files) are usually
                // transient, so back off briefly instead of shutting down.
                error!("Could not accept connection: {}", e);
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
        };
        trace!("Accepted connection from {}", remote_addr);
        let service = service.clone();
        tokio::spawn(async move {
            if let Err(e) = http1::Builder::new()
                .timer(TokioTimer::new())
                .serve_connection(TokioIo::new(stream), service)
                .await
            {
                debug!("Error serving connection from {}: {}", remote_addr, e);
            }
        });
    }
}
