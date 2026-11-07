use std::path::Path;
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;

pub const LOG_ENV: &str = "DROVER_LOG";

pub fn init(state: &Path) {
    if std::fs::create_dir_all(state).is_err() {
        return;
    }
    let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(state.join("drover.log"))
    else {
        return;
    };
    let filter = EnvFilter::try_from_env(LOG_ENV).unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .with_env_filter(filter)
        .try_init();
}
