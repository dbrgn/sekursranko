pub type Router = route_recognizer::Router<Route>;

/// All possible routes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Route {
    Index,
    Config,
    Backup,
}

/// Create a new router instance.
pub fn make_router() -> Router {
    let mut router = route_recognizer::Router::new();
    router.add("/", Route::Index);
    router.add("/config", Route::Config);
    router.add("/backups/:backupId", Route::Backup);
    router
}
