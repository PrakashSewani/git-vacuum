use std::time::{Duration, Instant};

use git_vacuum_app::AppState;
use git_vacuum_core::{Action, AppEvent, Effect};
use git_vacuum_tui;

mod config;
mod wiring;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .init();
    let cli = config::parse();

    let mut ctx = match wiring::create_app(&cli).await {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    };

    if cli.sync {
        println!("Non-interactive sync not yet implemented.");
        return Ok(());
    }

    let mut terminal = git_vacuum_tui::terminal::init()?;

    if let Some(token) = cli.token.clone() {
        ctx.app.reduce(Action::AuthSubmitToken(token));
    }

    let mut last_tick = Instant::now();
    let tick_rate = Duration::from_millis(16);

    loop {
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if crossterm::event::poll(timeout)? {
            match crossterm::event::read()? {
                crossterm::event::Event::Key(key) => {
                    let action = git_vacuum_tui::input::map_key_to_action(key, &ctx.app);
                    let effects = ctx.app.reduce(action);
                    execute_effects(effects, &ctx);
                }
                crossterm::event::Event::Paste(text) => {
                    let effects = ctx.app.reduce(Action::AuthSetToken(text));
                    execute_effects(effects, &ctx);
                }
                _ => {}
            }
        }

        let now = Instant::now();
        if now - last_tick >= tick_rate {
            ctx.app.tick_count += 1;
            last_tick = now;
        }

        while let Ok(event) = ctx.app.event_bus.app_rx.try_recv() {
            let effects = ctx.app.reduce_event(event);
            execute_effects(effects, &ctx);
        }
        while let Ok(event) = ctx.app.event_bus.progress_rx.try_recv() {
            let effects = ctx.app.reduce_event(event);
            execute_effects(effects, &ctx);
        }

        terminal.draw(|frame| {
            git_vacuum_tui::render(frame, &ctx.app);
        })?;

        if ctx.app.should_quit {
            break;
        }

        if let AppState::FatalError { .. } = &ctx.app.state {
            break;
        }
    }

    let _ = ctx.app.event_bus.cancel_tx.send(true);
    git_vacuum_tui::terminal::restore()?;

    Ok(())
}

fn execute_effects(effects: Vec<Effect>, ctx: &wiring::AppContext) {
    for effect in effects {
        match effect {
            Effect::None => {}

            Effect::AuthenticatePat { token } => {
                let github = ctx.services.github.clone();
                let db = ctx.services.db.clone();
                let keyring = ctx.services.keyring.clone();
                let app_tx = ctx.app.event_bus.app_tx.clone();
                let cancel_rx = ctx.app.event_bus.cancel_rx.clone();

                tokio::spawn(async move {
                    git_vacuum_service::authenticate_pat(
                        token, github, db, keyring, app_tx,
                    ).await;
                });
            }

            Effect::StartOAuthDeviceFlow { client_id, scopes } => {
                let github = ctx.services.github.clone();
                let app_tx = ctx.app.event_bus.app_tx.clone();

                tokio::spawn(async move {
                    match git_vacuum_service::device_flow_init(
                        client_id, scopes, github, app_tx,
                    ).await {
                        Ok(device_code) => {
                            // Start polling
                            // For now, just store the device code
                        }
                        Err(e) => {
                            log::error!("Device flow init failed: {}", e);
                        }
                    }
                });
            }

            Effect::DiscoverRepos { source } => {
                let github = ctx.services.github.clone();
                let db = ctx.services.db.clone();
                let app_tx = ctx.app.event_bus.app_tx.clone();
                let cancel_rx = ctx.app.event_bus.cancel_rx.clone();

                tokio::spawn(async move {
                    match git_vacuum_service::discover_repos(
                        source, github, db, app_tx, cancel_rx,
                    ).await {
                        Ok(_repos) => {}
                        Err(e) => {
                            log::error!("Discovery failed: {}", e);
                        }
                    }
                });
            }

            Effect::StartSync { repos, options, base_path } => {
                let git = ctx.services.git.clone();
                let db = ctx.services.db.clone();
                let progress_tx = ctx.app.event_bus.progress_tx.clone();
                let app_tx = ctx.app.event_bus.app_tx.clone();
                let cancel_rx = ctx.app.event_bus.cancel_rx.clone();

                tokio::spawn(async move {
                    match git_vacuum_service::run_sync(
                        repos, base_path, options, db, git,
                        progress_tx, app_tx, cancel_rx,
                    ).await {
                        Ok(summary) => {
                            log::info!("Sync completed: {} repos", summary.total_repos);
                        }
                        Err(e) => {
                            log::error!("Sync failed: {}", e);
                        }
                    }
                });
            }

            Effect::RefreshDashboardStats => {
                let app_tx = ctx.app.event_bus.app_tx.clone();
                let _ = app_tx.send(AppEvent::StatsRefreshed {
                    total_repos: 0,
                    up_to_date: 0,
                    behind: 0,
                    errors: 0,
                    total_size_bytes: 0,
                    attention_list: vec![],
                });
            }

            _ => {}
        }
    }
}
