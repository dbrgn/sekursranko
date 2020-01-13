use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use futures::future::{self, FutureExt};
use futures_fs::FsPool;
use hyper::{Body, Request, Response};
use hyper::service::Service;
use log::trace;

use crate::config::ServerConfig;
use crate::handlers::handler;

/// A `BackupService` wraps a configuration and a reference counted file system pool.
#[derive(Debug, Clone)]
pub struct BackupService {
    config: ServerConfig,
    fs_pool: Arc<FsPool>,
}

impl BackupService {
    pub fn new(config: ServerConfig) -> Self {
        let io_threads = config.io_threads;
        Self {
            config,
            fs_pool: Arc::new(FsPool::new(io_threads)),
        }
    }
}

impl Service<Request<Body>> for BackupService {
	type Response = Response<Body>;
	type Error = hyper::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        trace!("BackupService::call");
        // TODO: Find out how not to clone the config and the pool
        handler(req, self.config.clone(), self.fs_pool.clone()).boxed()
    }
}

/// The `MakeBackupService` is here to create an instance of `BackupService`
/// per connection. It does so by cloning the wrapped service.
#[derive(Debug)]
pub struct MakeBackupService(BackupService);

impl MakeBackupService {
    pub fn new(config: ServerConfig) -> Self {
        Self(BackupService::new(config))
    }
}

impl<T> Service<T> for MakeBackupService {
    type Response = BackupService;
    type Error = std::io::Error;
    type Future = future::Ready<Result<Self::Response, Self::Error>>;


    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _: T) -> Self::Future {
        trace!("MakeBackupService::call");
        future::ok(self.0.clone())
    }
}
