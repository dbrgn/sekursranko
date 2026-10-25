use std::{convert::Infallible, future::Future, pin::Pin, sync::Arc, time::Duration};

use http_body_util::Full;
use hyper::{
    Request, Response,
    body::{Bytes, Incoming},
    server::conn::http1,
    service::Service,
};
use hyper_util::rt::{TokioIo, TokioTimer};
use log::{debug, error, trace};
use tokio::net::TcpListener;

use crate::{config::ServerConfig, handlers::handler};

/// A `BackupService` wraps a configuration.
#[derive(Debug, Clone)]
pub struct BackupService {
    config: Arc<ServerConfig>,
}

impl BackupService {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config: Arc::new(config),
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

        // Copy Arc reference that will be moved into the future
        let config = self.config.clone();

        // Call handler
        Box::pin(async move { handler(req, &config).await })
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
