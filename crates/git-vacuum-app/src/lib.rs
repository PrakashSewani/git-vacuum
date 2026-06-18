pub mod effects;
pub mod modals;
pub mod reduce;
pub mod state;
pub mod tabs;

use std::sync::Arc;

use git_vacuum_core::{Effect, EventBusHandle};
use git_vacuum_service::Services;

/// Root application state. Owned exclusively by the main loop.
/// Only `reduce()` and `reduce_event()` mutate this.
pub struct App {
    pub state: state::AppState,
    pub should_quit: bool,
    pub terminal_size: (u16, u16),
    pub tick_count: u64,
    pub services: Arc<Services>,
    pub event_bus: EventBusHandle,
    pub pending_effects: Vec<Effect>,
    /// OAuth client_id (set on construction; required for the browser-sign-in flow)
    pub oauth_client_id: Option<String>,
}

impl App {
    pub fn new(
        services: Arc<Services>,
        event_bus: EventBusHandle,
        oauth_client_id: Option<String>,
    ) -> Self {
        let mut state = state::AppState::Auth(state::AuthScreenState::default());
        if let state::AppState::Auth(auth) = &mut state {
            auth.oauth_client_id = oauth_client_id.clone();
        }
        Self {
            state,
            should_quit: false,
            terminal_size: (0, 0),
            tick_count: 0,
            services,
            event_bus,
            pending_effects: Vec::new(),
            oauth_client_id,
        }
    }

    pub fn reduce(&mut self, action: git_vacuum_core::Action) {
        let effects = reduce::reduce_action(self, action);
        self.pending_effects.extend(effects);
    }

    pub fn reduce_event(&mut self, event: git_vacuum_core::AppEvent) {
        let effects = reduce::reduce_event(self, event);
        self.pending_effects.extend(effects);
    }

    pub fn drain_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.pending_effects)
    }
}
