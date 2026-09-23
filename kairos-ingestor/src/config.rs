use std::env;
use std::sync::OnceLock;

#[allow(unused)]
pub struct Config {
    pub rpc_url: String,
    pub database_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            rpc_url: require_env("RPC_URL"),
            database_url: require_env("DATABASE_URL"),
        }
    }

    /// Publishes `self` as the process-wide config. Must be called exactly once, before any
    /// `config::get()` call — `main` does this right after `Config::from_env()`.
    pub fn init(self) {
        CONFIG
            .set(self)
            .unwrap_or_else(|_| panic!("Config::init called more than once"));
    }
}

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the process-wide config set by `Config::init`. Panics if that hasn't run yet — every
/// task spawned after `main` calls it can rely on this instead of having `Config` threaded
/// through as a parameter.
pub fn get() -> &'static Config {
    CONFIG
        .get()
        .expect("Config::init must be called before config::get()")
}

fn require_env(key: &str) -> String {
    env::var(key).unwrap_or_else(|_| panic!("{key} must be set"))
}
