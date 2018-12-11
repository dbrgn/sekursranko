use std::sync::Arc;

use futures::future;
use futures_fs::FsPool;
use hyper::{Body, Request, Response};
use hyper::rt::Future;
use hyper::service::{Service, NewService};
use log::trace;

use crate::config::ServerConfig;
use crate::handlers::handler;

pub type ResponseFuture = Box<dyn Future<Item = Response<Body>, Error = hyper::Error> + Send>;

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

impl Service for BackupService {
    type ReqBody = Body;
	type ResBody = Body;
	type Error = hyper::Error;
    type Future = ResponseFuture;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        trace!("BackupService::call");
        handler(&req, &self.config, &self.fs_pool)
    }
}

impl NewService for BackupService {
    type ReqBody = Body;
    type ResBody = Body;
    type Error = hyper::Error;
    type InitError = hyper::Error;
    type Service = Self;
    type Future = Box<dyn Future<Item = Self::Service, Error = Self::InitError> + Send>;
    fn new_service(&self) -> Self::Future {
        trace!("BackupService::new_service");
        Box::new(future::ok(self.clone()))
    }
}
