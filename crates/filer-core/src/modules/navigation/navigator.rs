//! # Navigator Actor
//!
//! The navigator owns per-session history and coordinates location scans. It
//! normalizes incoming references through the registry before storing them so
//! navigation and refresh never depend on legacy node handles.

use std::sync::Arc;

use flume::{Receiver, Sender};
use rapidhash::fast::RandomState;

use crate::actors::Actor;
use crate::api::event_sink::EventSink;
use crate::model::directory::DirectoryLoadOptions;
use crate::model::location::{Location, LocationId, LocationRef, LocationRoute};
use crate::model::registry::NodeRegistry;
use crate::model::request::RequestId;
use crate::model::session::SessionId;
use crate::modules::scan::scanner::ScanCommand;
use crate::pipeline::PipelineConfig;
use crate::utils::channel::{send_or_warn, send_or_warn_async};
use crate::{CoreError, Event};

pub use super::state::{NavState, NavigatorState};

/// Commands handled by the navigator actor.
#[derive(Debug, Clone)]
pub enum NavCommand {
    /// Navigate to a provider-aware location.
    NavigateToLocation {
        session: SessionId,
        location: LocationRef,
        request: RequestId,
    },
    /// Go back in history.
    Back(SessionId, RequestId),
    /// Go forward in history.
    Forward(SessionId, RequestId),
    /// Go to the parent directory.
    Up(SessionId, RequestId),
    /// Refresh the current directory.
    Refresh(SessionId, RequestId),
    /// Update the entire pipeline configuration.
    SetPipeline {
        session: SessionId,
        config: PipelineConfig,
    },
    /// Replace or extend the current selection with provider-aware locations.
    SetSelected {
        session: SessionId,
        locations: Vec<LocationRef>,
    },
    /// Get the current state snapshot.
    GetState(SessionId),
    /// Invalidate a watched location.
    Invalidate(LocationRef),
    /// Remove session state after session destruction.
    RemoveSession(SessionId),
}

/// Navigator actor that coordinates navigation across sessions.
pub struct Navigator {
    commands: Receiver<NavCommand>,
    events: EventSink,
    scanner_tx: Sender<ScanCommand>,
    sessions: Arc<scc::HashMap<SessionId, NavigatorState, RandomState>>,
    location_cache: Arc<scc::HashSet<LocationId, RandomState>>,
    register: NodeRegistry,
}

impl Navigator {
    pub fn new<E: Into<EventSink>>(
        commands: Receiver<NavCommand>,
        events: E,
        scanner_tx: Sender<ScanCommand>,
        register: NodeRegistry,
    ) -> Self {
        Self {
            commands,
            events: events.into(),
            scanner_tx,
            sessions: Arc::new(scc::HashMap::with_hasher(RandomState::new())),
            location_cache: Arc::new(scc::HashSet::with_hasher(RandomState::new())),
            register,
        }
    }

    async fn get_or_init(&self, session: SessionId) {
        if !self.sessions.contains_async(&session).await {
            let _ = self
                .sessions
                .insert_async(session, NavigatorState::new())
                .await;
        }
    }

    async fn emit_snapshot(&self, session: SessionId) {
        let event = self
            .sessions
            .read_sync(&session, |_, state| Event::CurrentNavigateState {
                session,
                state: state.snapshot(),
            });
        if let Some(event) = event {
            send_or_warn_async(&self.events, event, "emit nav snapshot").await;
        }
    }

    async fn emit_navigation_error(
        &self,
        error: Option<CoreError>,
        session: SessionId,
        request: RequestId,
    ) {
        if let Some(error) = error {
            send_or_warn_async(
                &self.events,
                Event::from_request_error(error, session, request),
                "navigation unavailable",
            )
            .await;
        }
    }

    async fn handle_command(&self, command: NavCommand) {
        match command {
            NavCommand::NavigateToLocation {
                session,
                location,
                request,
            } => {
                self.get_or_init(session).await;
                let location = match self.register.resolve_location_ref(&location) {
                    Ok(location) => location,
                    Err(error) => {
                        send_or_warn_async(
                            &self.events,
                            Event::from_request_error(error, session, request),
                            "navigate resolve",
                        )
                        .await;
                        return;
                    }
                };
                let route = location.route();
                if matches!(route, LocationRoute::UnsupportedProvider { .. }) {
                    let Err(error) = route.require_direct_path() else {
                        return;
                    };
                    send_or_warn_async(
                        &self.events,
                        Event::from_request_error(error, session, request),
                        "navigate route",
                    )
                    .await;
                    return;
                }

                let location_ref = LocationRef::from_location(&location);
                let _ = self.location_cache.insert_async(location.id()).await;
                self.sessions
                    .update_async(&session, |_, state| {
                        state.navigate_location(location_ref.clone());
                        send_or_warn(
                            &self.scanner_tx,
                            ScanCommand::ScanLocation {
                                location: location_ref.clone(),
                                session,
                                pipeline: state.pipeline_config.clone(),
                                load: DirectoryLoadOptions::default(),
                                request,
                            },
                            "trigger location scan",
                        );
                    })
                    .await;
                self.emit_snapshot(session).await;
            }
            NavCommand::Back(session, request) => {
                self.get_or_init(session).await;
                let error = self
                    .sessions
                    .update_async(&session, |_, state| {
                        if state.can_back() {
                            let _ = state.back(1);
                            Self::trigger_current_scan(
                                session,
                                state,
                                self.scanner_tx.clone(),
                                request,
                            );
                            None
                        } else {
                            Some(CoreError::navigation_unavailable(
                                "Can't go back: no history",
                            ))
                        }
                    })
                    .await
                    .flatten();
                self.emit_navigation_error(error, session, request).await;
                self.emit_snapshot(session).await;
            }
            NavCommand::Forward(session, request) => {
                self.get_or_init(session).await;
                let error = self
                    .sessions
                    .update_async(&session, |_, state| {
                        if state.can_forward() {
                            let _ = state.forward();
                            Self::trigger_current_scan(
                                session,
                                state,
                                self.scanner_tx.clone(),
                                request,
                            );
                            None
                        } else {
                            Some(CoreError::navigation_unavailable(
                                "Can't go forward: no forward history",
                            ))
                        }
                    })
                    .await
                    .flatten();
                self.emit_navigation_error(error, session, request).await;
                self.emit_snapshot(session).await;
            }
            NavCommand::Up(session, request) => {
                self.get_or_init(session).await;
                let error = self
                    .sessions
                    .update_async(&session, |_, state| {
                        if let Some(location) = Self::parent_entry(state, &self.register) {
                            state.navigate_location(location);
                            Self::trigger_current_scan(
                                session,
                                state,
                                self.scanner_tx.clone(),
                                request,
                            );
                            None
                        } else {
                            Some(CoreError::navigation_unavailable(
                                "Can't go up: no parent directory",
                            ))
                        }
                    })
                    .await
                    .flatten();
                self.emit_navigation_error(error, session, request).await;
                self.emit_snapshot(session).await;
            }
            NavCommand::Refresh(session, request) => {
                self.get_or_init(session).await;
                let error = self
                    .sessions
                    .read_async(&session, |_key, state| {
                        if state.current.is_some() {
                            Self::trigger_current_refresh_scan(
                                session,
                                state,
                                self.scanner_tx.clone(),
                                request,
                            );
                            None
                        } else {
                            Some(CoreError::navigation_unavailable(
                                "Can't refresh: no current directory",
                            ))
                        }
                    })
                    .await
                    .flatten();
                self.emit_navigation_error(error, session, request).await;
            }
            NavCommand::SetPipeline { session, config } => {
                self.get_or_init(session).await;
                self.sessions
                    .update_async(&session, |_, state| {
                        state.pipeline_config = config;
                    })
                    .await;
                self.emit_snapshot(session).await;
            }
            NavCommand::SetSelected { session, locations } => {
                self.get_or_init(session).await;
                let normalized = locations
                    .into_iter()
                    .filter_map(|location| {
                        self.register
                            .resolve_location_ref(&location)
                            .ok()
                            .map(|resolved| LocationRef::from_location(&resolved))
                    })
                    .collect::<Vec<_>>();
                self.sessions
                    .update_async(&session, |_, state| {
                        for location in &normalized {
                            state.selected.insert(location.identity(), location.clone());
                        }
                    })
                    .await;
                self.emit_snapshot(session).await;
            }
            NavCommand::GetState(session) => {
                self.get_or_init(session).await;
                self.emit_snapshot(session).await;
            }
            NavCommand::Invalidate(location_ref) => {
                let Ok(location) = self.register.resolve_location_ref(&location_ref) else {
                    return;
                };
                let location_id = location.id();
                if !self.location_cache.contains_async(&location_id).await {
                    return;
                }
                self.sessions
                    .iter_async(|session, state| {
                        if state.current.as_ref().map(LocationRef::identity) == Some(location_id) {
                            Self::trigger_current_refresh_scan(
                                *session,
                                state,
                                self.scanner_tx.clone(),
                                RequestId::new(),
                            );
                        }
                        true
                    })
                    .await;
            }
            NavCommand::RemoveSession(session) => {
                let _ = self.sessions.remove_async(&session).await;
            }
        }
    }

    fn parent_entry(state: &NavigatorState, registry: &NodeRegistry) -> Option<LocationRef> {
        let current = state.current.as_ref()?;
        let location = registry.resolve_location_ref(current).ok()?;
        let LocationRoute::DirectPath { path } = location.route() else {
            return None;
        };
        let parent = path.parent()?.to_path_buf();
        Some(LocationRef::from_location(&Location::local(parent)))
    }

    fn trigger_current_scan(
        session: SessionId,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        let Some(location) = state.current.clone() else {
            return;
        };
        send_or_warn(
            &scanner_tx,
            ScanCommand::ScanLocation {
                location,
                session,
                pipeline: state.pipeline_config.clone(),
                load: DirectoryLoadOptions::default(),
                request,
            },
            "trigger location scan",
        );
    }

    fn trigger_current_refresh_scan(
        session: SessionId,
        state: &NavigatorState,
        scanner_tx: Sender<ScanCommand>,
        request: RequestId,
    ) {
        let Some(location) = state.current.clone() else {
            return;
        };
        send_or_warn(
            &scanner_tx,
            ScanCommand::RefreshLocation {
                location,
                session,
                pipeline: state.pipeline_config.clone(),
                load: DirectoryLoadOptions::default(),
                request,
            },
            "trigger location refresh",
        );
    }
}

impl Actor for Navigator {
    async fn run(self) {
        while let Ok(command) = self.commands.recv_async().await {
            self.handle_command(command).await;
        }
    }

    fn name(&self) -> &'static str {
        "navigator"
    }
}
