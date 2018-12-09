use hyper::{Server, rt::Future, rt::run as hyper_run};

use sekursranko::{BackupService, ServerConfig};

fn main() {
    let port = 3000;
    let addr = ([127, 0, 0, 1], port).into();
    println!("Starting server on port {}", port);

    // Create server
    let config: ServerConfig = ServerConfig {
        max_backup_bytes: 524288,
        retention_days: 180,
        backup_dir: "backups".into(),
        io_threads: 4,
    };
    let context = BackupService::new(config);
    let server = Server::bind(&addr)
        .serve(context)
        .map_err(|e| eprintln!("Server error: {}", e));

    // Loop forever
    hyper_run(server);
}
