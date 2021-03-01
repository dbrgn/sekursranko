use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use hyper::{service::Service, Body, Request, Response};
use log::trace;

use crate::{
    config::ServerConfig,
    handlers::handler,
    routing::{make_router, Router},
};

// Note: Implementation based on `service_struct_impl.rs` example in the hyper repo.

/// A `BackupService` wraps a configuration and a reference counted file system pool.
#[derive(Debug, Clone)]
pub struct BackupService {
    config: Arc<ServerConfig>,
    router: Arc<Router>,
}

type PinBox<T> = Pin<Box<T>>;

impl Service<Request<Body>> for BackupService {
    type Response = Response<Body>;
    type Error = hyper::Error;
    type Future = PinBox<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        trace!("BackupService::call");

        // Copy Arc references that will be moved into the future
        let config = self.config.clone();
        let router = self.router.clone();

        // Call handler
        Box::pin(async move { handler(req, &router, &config).await })
    }
}

pub struct MakeBackupService {
    config: Arc<ServerConfig>,
    router: Arc<Router>,
}

impl MakeBackupService {
    pub fn new(config: ServerConfig) -> Self {
        Self {
            config: Arc::new(config),
            router: Arc::new(make_router()),
        }
    }
}

impl<T> Service<T> for MakeBackupService {
    type Response = BackupService;
    type Error = hyper::Error;
    type Future = PinBox<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>;

    fn poll_ready(&mut self, _: &mut Context) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        let config = self.config.clone();
        let router = self.router.clone();
        let fut = async move { Ok(BackupService { config, router }) };
        Box::pin(fut)
    }
}
