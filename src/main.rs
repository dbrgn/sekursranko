use env_logger;
use hyper::{Server, rt::Future, rt::run as hyper_run};
use log::{info, error};

use sekursranko::{BackupService, ServerConfig};

fn main() {
    env_logger::init();

    let port = 3000;
    let addr = ([127, 0, 0, 1], port).into();
    info!("Starting server on port {}", port);

    // Create server
    let config: ServerConfig = ServerConfig {
        max_backup_bytes: 512 * 1024,
        retention_days: 180,
        backup_dir: "backups".into(),
        io_threads: 4,
    };
    let context = BackupService::new(config);
    let server = Server::bind(&addr)
        .serve(context)
        .map_err(|e| error!("Server error: {}", e));

    // Loop forever
    hyper_run(server);
}
