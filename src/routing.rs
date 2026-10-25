/// All possible routes.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Route<'a> {
    Index,
    Config,
    Backup { backup_id: &'a str },
}

/// Match a request path against all known routes.
pub fn recognize(path: &str) -> Option<Route<'_>> {
    let path = path.strip_prefix('/').unwrap_or(path);
    match path {
        "" => Some(Route::Index),
        "config" => Some(Route::Config),
        _ => match path.strip_prefix("backups/") {
            Some(backup_id) if !backup_id.is_empty() && !backup_id.contains('/') => {
                Some(Route::Backup { backup_id })
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognize_static_routes() {
        assert_eq!(recognize("/"), Some(Route::Index));
        assert_eq!(recognize(""), Some(Route::Index));
        assert_eq!(recognize("/config"), Some(Route::Config));
        assert_eq!(recognize("config"), Some(Route::Config));
    }

    #[test]
    fn recognize_backup_route() {
        for backup_id in ["a", "abc123", "a.b", "%2F", "ä"] {
            assert_eq!(
                recognize(&format!("/backups/{}", backup_id)),
                Some(Route::Backup { backup_id })
            );
        }
        assert_eq!(
            recognize("backups/abc"),
            Some(Route::Backup { backup_id: "abc" })
        );
    }

    #[test]
    fn recognize_unknown_routes() {
        for path in [
            "//",
            "/config/",
            "//config",
            "/configx",
            "/Config",
            "/backups",
            "/backups/",
            "/backups/a/b",
            "/backups/a/",
            "/backups//a",
            "//backups/a",
            "/backupsx/a",
            "/index",
        ] {
            assert_eq!(recognize(path), None, "path {:?}", path);
        }
    }
}
