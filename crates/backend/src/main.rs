mod acp;
mod cli;
mod commands;
mod fetch_cycle;
mod fetchers;
mod kb_search;
mod vault_organize;
mod process;
mod scheduler;
mod server;
mod trello;
pub mod state;
pub mod util;

use std::fs;
use tokio::net::UnixListener;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use state::AppState;
use util::{log, log_dir};

// Re-export for use as crate::X in submodules
pub use util::get_rss_mb;
pub use state::EventTx;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if let Some(err) = pasta_common::config::init() {
        eprintln!("config: {}", err);
    }

    let log_level = &pasta_common::config::get().general.log_level;
    let log_file = fs::OpenOptions::new()
        .create(true).append(true)
        .open(log_dir().join("tracing.log"))?;
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level)))
        .with(fmt::layer().with_ansi(false).with_writer(std::sync::Mutex::new(log_file)))
        .init();

    if pasta_common::config::get().schedules.contains_key("kb-sync") {
        tracing::warn!("[schedules.kb-sync] is deprecated — kb-sync now runs inline in the fetch cycle");
    }

    // Set gog env vars from config
    let gog = &pasta_common::config::get().gog;
    if !gog.account.is_empty() { std::env::set_var("GOG_ACCOUNT", &gog.account); }
    if !gog.home.is_empty() { std::env::set_var("GOG_HOME", &gog.home); }
    if !gog.keyring_backend.is_empty() { std::env::set_var("GOG_KEYRING_BACKEND", &gog.keyring_backend); }
    if !gog.keyring_password.is_empty() { std::env::set_var("GOG_KEYRING_PASSWORD", &gog.keyring_password); }

    tokio::spawn(async {
        let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop { tick.tick().await; tracing::info!(rss_mb = get_rss_mb(), "memory"); }
    });

    let args: Vec<String> = std::env::args().collect();
    let has = |flag: &str| args.iter().any(|a| a == flag);

    if has("--organize") { return cli::organize().await; }
    if has("--daily") { return cli::daily().await; }
    if has("--weekly") { return cli::weekly().await; }
    if has("--fetch") {
        let completed = fetch_cycle::run(std::collections::HashSet::new(), None).await;
        eprintln!("  ✓ Fetch cycle complete ({} fetchers)", completed.len());
        return Ok(());
    }
    if has("--route-tasks") { return cli::route_tasks().await; }
    if has("--sync-trello") { return cli::sync_trello().await; }

    // --- Daemon mode ---
    let lock_path = dirs::home_dir().unwrap().join(".pasta/pasta.lock");
    fs::create_dir_all(lock_path.parent().unwrap())?;
    let lock_file = fs::OpenOptions::new().create(true).read(true).write(true).truncate(false).open(&lock_path)?;
    use std::os::unix::io::AsRawFd;
    if unsafe { libc::flock(lock_file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        eprintln!("Another pasta-backend is already running");
        std::process::exit(1);
    }

    let sock_path = pasta_common::ipc::socket_path();
    if sock_path.exists() { fs::remove_file(&sock_path)?; }
    fs::create_dir_all(sock_path.parent().unwrap())?;

    let listener = UnixListener::bind(&sock_path)?;
    log("backend", &format!("listening on {}", sock_path.display()));

    let state = AppState::new();
    scheduler::spawn(state.clone());

    let shutdown_state = state.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        log("backend", "shutdown signal received");
        let handles: Vec<_> = shutdown_state.process.lock().await.running.drain().map(|(_, h)| h).collect();
        for h in &handles { h.abort(); }
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5),
            futures::future::join_all(handles)).await;
        std::process::exit(0);
    });

    server::run(listener, state).await;
}
