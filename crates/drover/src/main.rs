use std::process::ExitCode;

use drover::app::{Msg, Options};
use drover::cli::{self, Command, USAGE};
use drover::runtime::Runner;
use drover::{host, logging};
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;

fn main() -> ExitCode {
    let command = match cli::parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => {
            eprintln!("drover: {error}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let paths = config::Paths::from_env();
    match command {
        Command::Help => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        Command::Version => {
            println!("drover {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Command::DefaultConfig => {
            print!("{}", config::default_config_text());
            ExitCode::SUCCESS
        }
        Command::ConfigCheck => check(&paths),
        Command::ResetKeys => reset_keys(&paths),
        Command::Run { session } => run(&paths, session),
    }
}

fn check(paths: &config::Paths) -> ExitCode {
    match config::check(&paths.config) {
        Ok(()) => {
            println!("config: ok ({})", paths.config.display());
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            println!("config: issues found ({})", paths.config.display());
            for diagnostic in diagnostics {
                println!("  {diagnostic}");
            }
            ExitCode::FAILURE
        }
    }
}

fn reset_keys(paths: &config::Paths) -> ExitCode {
    match config::reset_keys(&paths.config) {
        Ok(config::ResetKeys::NoFile) => {
            println!("no config file at {}", paths.config.display());
            ExitCode::SUCCESS
        }
        Ok(config::ResetKeys::NoKeys) => {
            println!("no key settings in {}", paths.config.display());
            ExitCode::SUCCESS
        }
        Ok(config::ResetKeys::Reset { backup }) => {
            println!(
                "reset keys in {}; backup at {}",
                paths.config.display(),
                backup.display()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("drover: cannot reset keys: {error}");
            ExitCode::FAILURE
        }
    }
}

fn hostname() -> String {
    rustix::system::uname()
        .nodename()
        .to_string_lossy()
        .split('.')
        .next()
        .unwrap_or_default()
        .to_owned()
}

fn run(paths: &config::Paths, session: Option<String>) -> ExitCode {
    let mut notes = Vec::new();
    match paths.migrate() {
        Ok(Some(migration)) => {
            let note = format!(
                "copied {} to {}",
                migration.from.display(),
                migration.to.display()
            );
            eprintln!("drover: {note}");
            notes.push(note);
        }
        Ok(None) => {}
        Err(error) => eprintln!("drover: cannot copy the Herdr config: {error}"),
    }
    let loaded = config::Config::load(&paths.config);
    let caller = std::env::var("HC_TERMINAL_ID")
        .ok()
        .filter(|id| !id.is_empty());
    if let Some(id) = &caller
        && !loaded.config.experimental.allow_nested
    {
        eprintln!(
            "drover: this shell runs inside hc terminal {id}; set experimental.allow_nested = true in {} to open drover here",
            paths.config.display()
        );
        return ExitCode::FAILURE;
    }
    logging::init(&paths.state);
    tracing::info!(version = env!("CARGO_PKG_VERSION"), "drover starting");
    let mut env = hc::Env::current();
    if let Some(name) = &session {
        env.set("HC_SESSION", name);
    }
    let session_name = session
        .clone()
        .or_else(|| std::env::var("HC_SESSION").ok())
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "default".to_owned());
    let prefs_path = paths.preferences(&session_name);
    host::install_panic_hook();
    let guard = match host::enter() {
        Ok(guard) => guard,
        Err(error) => {
            host::restore();
            eprintln!("drover: cannot set up the terminal: {error}");
            return ExitCode::FAILURE;
        }
    };
    let info = host::probe();
    let options = Options {
        env,
        config_path: paths.config.clone(),
        prefs_path,
        loaded,
        host: info,
        caller,
        notes,
        hostname: hostname(),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            drop(guard);
            eprintln!("drover: {error}");
            return ExitCode::FAILURE;
        }
    };
    let local = tokio::task::LocalSet::new();
    let result = local.block_on(&runtime, async move {
        let backend = CrosstermBackend::new(std::io::stdout());
        let mut runner = Runner::new(options, backend, Box::new(std::io::stdout()))?;
        let tx = runner.sender();
        tokio::task::spawn_local(async move {
            let mut events = crossterm::event::EventStream::new();
            while let Some(event) = events.next().await {
                match event {
                    Ok(event) => {
                        if tx.send(Msg::Host(event)).is_err() {
                            return;
                        }
                    }
                    Err(error) => {
                        tracing::error!(%error, "host input failed");
                        break;
                    }
                }
            }
            let _ = tx.send(Msg::Quit);
        });
        let signals = runner.sender();
        tokio::task::spawn_local(async move {
            use tokio::signal::unix::{SignalKind, signal};
            let (Ok(mut term), Ok(mut hup), Ok(mut int)) = (
                signal(SignalKind::terminate()),
                signal(SignalKind::hangup()),
                signal(SignalKind::interrupt()),
            ) else {
                return;
            };
            tokio::select! {
                _ = term.recv() => {}
                _ = hup.recv() => {}
                _ = int.recv() => {}
            }
            let _ = signals.send(Msg::Quit);
        });
        runner.run().await;
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        Ok::<(), std::io::Error>(())
    });
    drop(guard);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("drover: {error}");
            ExitCode::FAILURE
        }
    }
}
