impl ChatMode {
    fn displayed_chat_state<'a>(&'a self, root: &'a ChatState) -> &'a ChatState {
        self.lineage_inspection
            .as_ref()
            .map(|inspection| &inspection.chat_state)
            .unwrap_or(root)
    }

    fn project_inspected_lineage_event(&mut self, event: &AgenticEvent) -> bool {
        let Some(event_session_id) = event.session_id() else {
            return false;
        };
        let Some(lineage_active_turn_id) = self.lineage_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .sessions
                .iter()
                .find(|entry| entry.session_id == event_session_id)
                .map(|entry| entry.active_turn_id.clone())
        }) else {
            return false;
        };
        match event {
            AgenticEvent::DialogTurnStarted { turn_id, .. } => {
                if lineage_terminal_reconciliation_pending(
                    self.lineage_inspection.as_ref(),
                    event_session_id,
                ) {
                    if let Some(inspection) = self.lineage_inspection.as_mut() {
                        let now = Instant::now();
                        inspection.refresh_pending = true;
                        inspection.refresh_due_at = now;
                        inspection.refresh_deadline = Some(now + LINEAGE_SETTLEMENT_RETRY_WINDOW);
                    }
                } else {
                    self.lineage_settlements.remove(event_session_id);
                    self.retain_active_lineage_events(event_session_id, Some(turn_id));
                    clear_lineage_settlement_for_new_turn(
                        self.lineage_inspection.as_mut(),
                        event_session_id,
                    );
                }
            }
            AgenticEvent::DialogTurnCompleted { .. }
            | AgenticEvent::DialogTurnFailed { .. }
            | AgenticEvent::DialogTurnCancelled { .. } => {
                let Some(turn_id) = event.turn_id() else {
                    return false;
                };
                let observed_turn = lineage_active_turn_id.as_deref() == Some(turn_id)
                    || self.lineage_event_buffer.iter().any(|buffered| {
                        buffered.event.session_id() == Some(event_session_id)
                            && buffered.event.turn_id() == Some(turn_id)
                    })
                    || self.lineage_inspection.as_ref().is_some_and(|inspection| {
                        inspection.selected_session_id == event_session_id
                            && inspection.chat_state.current_turn_id() == Some(turn_id)
                    });
                let Some(settlement) = lineage_settlement_from_event(
                    self.lineage_settlements.get(event_session_id),
                    observed_turn,
                    event,
                ) else {
                    return false;
                };
                self.lineage_settlements
                    .insert(event_session_id.to_string(), settlement);
            }
            _ => {}
        }
        let should_buffer = self.lineage_snapshot.as_ref().is_some_and(|snapshot| {
            snapshot.sessions.iter().any(|entry| {
                entry.session_id == event_session_id
                    && event.turn_id().is_some_and(|turn_id| {
                        matches!(event, AgenticEvent::DialogTurnStarted { .. })
                            || entry.active_turn_id.as_deref() == Some(turn_id)
                    })
            }) && is_buffered_lineage_event(event)
        });
        if should_buffer {
            push_bounded_lineage_event(
                &mut self.lineage_event_buffer,
                &mut self.lineage_event_buffer_bytes,
                event,
                LINEAGE_EVENT_BUFFER_MAX_BYTES,
                LINEAGE_EVENT_BUFFER_MAX_EVENTS,
            );
        }
        if let Some(snapshot) = self.lineage_snapshot.as_mut() {
            update_lineage_active_turn(snapshot, event);
        }
        let Some(inspection) = self
            .lineage_inspection
            .as_mut()
            .filter(|inspection| inspection.selected_session_id == event_session_id)
        else {
            return false;
        };

        let requires_authoritative_refresh = matches!(
            event,
            AgenticEvent::DialogTurnCompleted { .. }
                | AgenticEvent::DialogTurnFailed { .. }
                | AgenticEvent::DialogTurnCancelled { .. }
                | AgenticEvent::SessionHistoryChanged { .. }
        );
        let projection = project_transcript_event(&mut inspection.chat_state, event, false);
        if requires_authoritative_refresh {
            let now = Instant::now();
            inspection.refresh_pending = true;
            inspection.refresh_due_at = now;
            inspection.refresh_deadline = Some(now + LINEAGE_SETTLEMENT_RETRY_WINDOW);
            if matches!(
                event,
                AgenticEvent::DialogTurnCompleted { .. }
                    | AgenticEvent::DialogTurnFailed { .. }
                    | AgenticEvent::DialogTurnCancelled { .. }
            ) {
                if let Some(turn_id) = event.turn_id() {
                    inspection.settling_turn_ids.insert(turn_id.to_string());
                }
            }
            if matches!(
                event,
                AgenticEvent::DialogTurnFailed { .. } | AgenticEvent::DialogTurnCancelled { .. }
            ) {
                inspection.preserve_live_terminal = true;
            }
        }
        projection.changed || requires_authoritative_refresh
    }

    fn show_session_lineage(
        &mut self,
        chat_view: &mut ChatView,
        root_chat_state: &ChatState,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let root_session_id = self
            .lineage_snapshot
            .as_ref()
            .map(|snapshot| snapshot.root_session_id.as_str())
            .unwrap_or(&root_chat_state.core_session_id)
            .to_string();
        let agent = self.agent.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.session_lineage(&root_session_id))
        });
        match result {
            Ok(Some(snapshot))
                if snapshot
                    .sessions
                    .iter()
                    .any(|entry| entry.session_id != snapshot.root_session_id) =>
            {
                chat_view.show_session_lineage_selector(&snapshot);
                chat_view.set_status(Some(
                    "Select a subagent Session to inspect its transcript".to_string(),
                ));
                self.lineage_snapshot = Some(snapshot);
            }
            Ok(_) => chat_view.set_status(Some(
                "No subagent Sessions are available for this conversation".to_string(),
            )),
            Err(error) => chat_view.set_status(Some(format!("Could not load subagents: {error}"))),
        }
    }

    fn inspect_lineage_session(
        &mut self,
        session_id: &str,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let Some(snapshot) = self.lineage_snapshot.as_ref() else {
            chat_view.set_status(Some("Reopen View subagents and try again".to_string()));
            return;
        };
        let Some(entry) = snapshot
            .sessions
            .iter()
            .find(|entry| entry.session_id == session_id)
            .cloned()
        else {
            chat_view.set_status(Some(
                "That subagent Session is no longer in the current lineage".to_string(),
            ));
            return;
        };
        if entry.session_id == snapshot.root_session_id {
            self.leave_lineage_inspection(chat_view);
            return;
        }
        let root_session_id = snapshot.root_session_id.clone();
        let agent = self.agent.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.inspect_lineage_session(&root_session_id, session_id))
        });
        match result {
            Ok(inspection) => {
                let active_turn_id = inspection.active_turn_id.clone();
                let settlement = self.lineage_settlements.get(&entry.session_id).cloned();
                let stale = settlement.as_ref().is_some_and(|settlement| {
                    active_turn_id.as_deref() == Some(settlement.turn_id.as_str())
                });
                let candidate_settlement = settlement
                    .as_ref()
                    .filter(|settlement| stale || settlement.preserve_live_terminal);
                let replay_turn_id = candidate_settlement
                    .map(|settlement| settlement.turn_id.as_str())
                    .or(active_turn_id.as_deref());
                let replay_events = replay_turn_id
                    .map(|turn_id| self.buffered_lineage_events(&entry.session_id, turn_id))
                    .unwrap_or_default();
                let display_settlement = candidate_settlement.filter(|settlement| {
                    replay_events
                        .iter()
                        .any(|event| is_terminal_lineage_event(event, &settlement.turn_id))
                });
                let preserve_cached_terminal =
                    display_settlement.is_some_and(|settlement| settlement.preserve_live_terminal);
                let runtime_advanced_past_settlement =
                    display_settlement.is_some_and(|settlement| {
                        active_turn_id
                            .as_deref()
                            .is_some_and(|active_turn_id| active_turn_id != settlement.turn_id)
                    });
                let state = if runtime_advanced_past_settlement {
                    let runtime_replay_events = active_turn_id
                        .as_deref()
                        .map(|turn_id| self.buffered_lineage_events(&entry.session_id, turn_id))
                        .unwrap_or_default();
                    let mut runtime_state = build_lineage_chat_state(
                        &entry,
                        inspection.clone(),
                        &runtime_replay_events,
                    );
                    let settlement = display_settlement.expect("settlement checked above");
                    let mut replay_inspection = inspection;
                    replay_inspection.active_turn_id = Some(settlement.turn_id.clone());
                    let replayed =
                        build_lineage_chat_state(&entry, replay_inspection, &replay_events);
                    merge_live_terminal_into_authoritative(
                        &mut runtime_state,
                        &replayed,
                        None,
                        &settlement.turn_id,
                    );
                    runtime_state
                } else {
                    let mut display_inspection = inspection;
                    if let Some(settlement) = display_settlement {
                        display_inspection.active_turn_id = Some(settlement.turn_id.clone());
                    }
                    build_lineage_chat_state(&entry, display_inspection, &replay_events)
                };
                if let Some(snapshot) = self.lineage_snapshot.as_mut() {
                    if let Some(snapshot_entry) = snapshot
                        .sessions
                        .iter_mut()
                        .find(|candidate| candidate.session_id == entry.session_id)
                    {
                        snapshot_entry.active_turn_id =
                            if stale { None } else { active_turn_id.clone() };
                    }
                }
                let now = Instant::now();
                self.lineage_inspection = Some(AgentSessionInspection {
                    selected_session_id: entry.session_id.clone(),
                    chat_state: state,
                    refresh_pending: stale,
                    refresh_due_at: if stale {
                        now + Duration::from_millis(250)
                    } else {
                        now
                    },
                    refresh_deadline: stale.then_some(now + LINEAGE_SETTLEMENT_RETRY_WINDOW),
                    settling_turn_ids: settlement
                        .as_ref()
                        .filter(|_| stale)
                        .map(|settlement| BTreeSet::from([settlement.turn_id.clone()]))
                        .unwrap_or_default(),
                    preserve_live_terminal: settlement
                        .as_ref()
                        .is_some_and(|_| stale && preserve_cached_terminal),
                });
                if runtime_advanced_past_settlement {
                    self.lineage_settlements.remove(&entry.session_id);
                    self.retain_active_lineage_events(&entry.session_id, active_turn_id.as_deref());
                } else if !stale && !preserve_cached_terminal {
                    self.lineage_settlements.remove(&entry.session_id);
                    self.retain_active_lineage_events(&entry.session_id, active_turn_id.as_deref());
                } else if !stale {
                    self.retain_active_lineage_events(
                        &entry.session_id,
                        settlement.as_ref().map(|value| value.turn_id.as_str()),
                    );
                }
                chat_view.set_lineage_inspection(Some(entry.session_name));
                chat_view.invalidate_lines_cache();
                chat_view.set_status(Some(
                    "Read-only subagent transcript; root input remains preserved".to_string(),
                ));
            }
            Err(error) if error.outcome_unknown() => {
                let now = Instant::now();
                let settlement = self.lineage_settlements.get(&entry.session_id).cloned();
                let settlement_turn_id = settlement
                    .as_ref()
                    .map(|settlement| settlement.turn_id.as_str());
                let settlement_events = settlement_turn_id
                    .map(|turn_id| self.buffered_lineage_events(&entry.session_id, turn_id))
                    .unwrap_or_default();
                let has_terminal_event = settlement.as_ref().is_some_and(|settlement| {
                    settlement_events
                        .iter()
                        .any(|event| is_terminal_lineage_event(event, &settlement.turn_id))
                });
                let replay_turn_id = provisional_lineage_replay_turn(
                    settlement_turn_id,
                    has_terminal_event,
                    entry.active_turn_id.as_deref(),
                );
                let replay_events = replay_turn_id
                    .map(|turn_id| self.buffered_lineage_events(&entry.session_id, turn_id))
                    .unwrap_or_default();
                let state = build_lineage_chat_state(
                    &entry,
                    AgentSessionLineageInspection {
                        transcript: SessionTranscript {
                            session_id: entry.session_id.clone(),
                            messages: Vec::new(),
                        },
                        active_turn_id: replay_turn_id.map(str::to_string),
                    },
                    &replay_events,
                );
                self.lineage_inspection = Some(AgentSessionInspection {
                    selected_session_id: entry.session_id.clone(),
                    chat_state: state,
                    refresh_pending: true,
                    refresh_due_at: now + Duration::from_millis(250),
                    refresh_deadline: Some(now + LINEAGE_SETTLEMENT_RETRY_WINDOW),
                    settling_turn_ids: settlement
                        .as_ref()
                        .map(|settlement| BTreeSet::from([settlement.turn_id.clone()]))
                        .unwrap_or_default(),
                    preserve_live_terminal: settlement.as_ref().is_some_and(|settlement| {
                        has_terminal_event && settlement.preserve_live_terminal
                    }),
                });
                chat_view.set_lineage_inspection(Some(entry.session_name));
                chat_view.invalidate_lines_cache();
                chat_view.set_status(Some(
                    "Waiting for the subagent transcript to settle".to_string(),
                ));
            }
            Err(error) => {
                chat_view.set_status(Some(format!("Could not inspect subagent Session: {error}")))
            }
        }
    }

    fn refresh_inspected_lineage_if_due(
        &mut self,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) -> bool {
        let now = Instant::now();
        let Some(selected_session_id) = self.lineage_inspection.as_ref().and_then(|inspection| {
            (inspection.refresh_pending && inspection.refresh_due_at <= now)
                .then(|| inspection.selected_session_id.clone())
        }) else {
            return false;
        };
        let Some(snapshot) = self.lineage_snapshot.as_ref() else {
            return false;
        };
        let root_session_id = snapshot.root_session_id.clone();
        let Some(entry) = snapshot
            .sessions
            .iter()
            .find(|entry| entry.session_id == selected_session_id)
            .cloned()
        else {
            if let Some(inspection) = self.lineage_inspection.as_mut() {
                inspection.refresh_pending = false;
            }
            chat_view.set_status(Some(
                "The inspected subagent is no longer in this lineage".to_string(),
            ));
            return true;
        };

        let agent = self.agent.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle
                .block_on(agent.inspect_lineage_session(&root_session_id, &selected_session_id))
        });
        match result {
            Ok(inspection) => {
                let active_turn_id = inspection.active_turn_id.clone();
                let refresh_action = self
                    .lineage_inspection
                    .as_ref()
                    .map(|current| {
                        lineage_refresh_action(
                            &current.settling_turn_ids,
                            active_turn_id.as_deref(),
                            current.preserve_live_terminal,
                        )
                    })
                    .unwrap_or(LineageRefreshAction::ReplaceFromRuntime);
                if refresh_action == LineageRefreshAction::RetrySettlement {
                    if let Some(current) = self.lineage_inspection.as_mut() {
                        if current
                            .refresh_deadline
                            .is_some_and(|deadline| Instant::now() < deadline)
                        {
                            current.refresh_pending = true;
                            current.refresh_due_at = Instant::now() + Duration::from_millis(250);
                            return false;
                        }
                        current.refresh_pending = false;
                        current.refresh_deadline = None;
                    }
                    chat_view.set_status(Some(
                        "The subagent transcript is still settling; reopen View subagents to retry"
                            .to_string(),
                    ));
                    return true;
                }
                let preserved_turn_id = (refresh_action
                    == LineageRefreshAction::PreserveLiveTerminal)
                    .then(|| {
                        self.lineage_settlements
                            .get(&selected_session_id)
                            .map(|settlement| settlement.turn_id.clone())
                            .or_else(|| {
                                self.lineage_inspection.as_ref().and_then(|current| {
                                    current.settling_turn_ids.iter().next().cloned()
                                })
                            })
                    })
                    .flatten();
                let replay_events = active_turn_id
                    .as_deref()
                    .map(|turn_id| self.buffered_lineage_events(&selected_session_id, turn_id))
                    .unwrap_or_default();
                let preserved_projection = preserved_turn_id.as_ref().map(|turn_id| {
                    let preserved_events =
                        self.buffered_lineage_events(&selected_session_id, turn_id);
                    let mut preserved_inspection = inspection.clone();
                    preserved_inspection.active_turn_id = Some(turn_id.clone());
                    build_lineage_chat_state(&entry, preserved_inspection, &preserved_events)
                });
                let mut runtime_state =
                    build_lineage_chat_state(&entry, inspection, &replay_events);
                if let Some(snapshot_entry) = self.lineage_snapshot.as_mut().and_then(|snapshot| {
                    snapshot
                        .sessions
                        .iter_mut()
                        .find(|candidate| candidate.session_id == selected_session_id)
                }) {
                    snapshot_entry.active_turn_id = active_turn_id.clone();
                }
                if let Some(current) = self
                    .lineage_inspection
                    .as_mut()
                    .filter(|inspection| inspection.selected_session_id == selected_session_id)
                {
                    if refresh_action == LineageRefreshAction::PreserveLiveTerminal {
                        if let Some(turn_id) = preserved_turn_id.as_deref() {
                            merge_live_terminal_into_authoritative(
                                &mut runtime_state,
                                &current.chat_state,
                                preserved_projection.as_ref(),
                                turn_id,
                            );
                        }
                    }
                    current.chat_state = runtime_state;
                    current.refresh_pending = false;
                    current.refresh_due_at = now;
                    current.refresh_deadline = None;
                    current.settling_turn_ids.clear();
                    current.preserve_live_terminal = false;
                }
                if refresh_action == LineageRefreshAction::PreserveLiveTerminal
                    && active_turn_id.is_none()
                {
                    self.retain_active_lineage_events(
                        &selected_session_id,
                        preserved_turn_id.as_deref(),
                    );
                } else {
                    self.lineage_settlements.remove(&selected_session_id);
                    self.retain_active_lineage_events(
                        &selected_session_id,
                        active_turn_id.as_deref(),
                    );
                }
                chat_view.set_status(Some(
                    "Read-only subagent transcript; root input remains preserved".to_string(),
                ));
                true
            }
            Err(error) => {
                let retryable = error.outcome_unknown();
                if let Some(current) = self
                    .lineage_inspection
                    .as_mut()
                    .filter(|inspection| inspection.selected_session_id == selected_session_id)
                {
                    // Only the Runtime's typed settlement uncertainty is
                    // retryable. Permanent storage/workspace errors stop here.
                    if retryable
                        && current
                            .refresh_deadline
                            .is_some_and(|deadline| Instant::now() < deadline)
                    {
                        current.refresh_pending = true;
                        current.refresh_due_at = Instant::now() + Duration::from_millis(250);
                    } else {
                        current.refresh_pending = false;
                        current.refresh_deadline = None;
                    }
                }
                chat_view.set_status(Some(format!("Could not refresh subagent Session: {error}")));
                true
            }
        }
    }

    fn leave_lineage_inspection(&mut self, chat_view: &mut ChatView) {
        if self.lineage_inspection.take().is_some() {
            chat_view.set_lineage_inspection(None);
            chat_view.invalidate_lines_cache();
            chat_view.set_status(Some("Returned to the root conversation".to_string()));
        }
    }

    fn buffered_lineage_events(&self, session_id: &str, turn_id: &str) -> Vec<AgenticEvent> {
        self.lineage_event_buffer
            .iter()
            .filter(|buffered| {
                buffered.event.session_id() == Some(session_id)
                    && buffered.event.turn_id() == Some(turn_id)
            })
            .map(|buffered| buffered.event.clone())
            .collect()
    }

    fn retain_active_lineage_events(&mut self, session_id: &str, active_turn_id: Option<&str>) {
        self.lineage_event_buffer.retain(|buffered| {
            buffered.event.session_id() != Some(session_id)
                || active_turn_id.is_some_and(|turn_id| buffered.event.turn_id() == Some(turn_id))
        });
        self.lineage_event_buffer_bytes = self
            .lineage_event_buffer
            .iter()
            .map(|buffered| buffered.encoded_bytes)
            .sum();
    }

    fn reset_lineage_navigation(&mut self, chat_view: &mut ChatView) {
        self.lineage_snapshot = None;
        self.lineage_inspection = None;
        self.lineage_event_buffer.clear();
        self.lineage_event_buffer_bytes = 0;
        self.lineage_settlements.clear();
        chat_view.set_lineage_inspection(None);
    }

    fn navigate_lineage_parent(
        &mut self,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let Some(selected_session_id) = self
            .lineage_inspection
            .as_ref()
            .map(|inspection| inspection.selected_session_id.clone())
        else {
            return;
        };
        let Some(snapshot) = self.lineage_snapshot.as_ref() else {
            self.leave_lineage_inspection(chat_view);
            return;
        };
        let parent_session_id = lineage_parent_session_id(snapshot, &selected_session_id);
        match parent_session_id {
            Some(parent) if parent != snapshot.root_session_id => {
                self.inspect_lineage_session(&parent, chat_view, rt_handle)
            }
            _ => self.leave_lineage_inspection(chat_view),
        }
    }

    fn navigate_lineage_sibling(
        &mut self,
        offset: isize,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let Some(selected_session_id) = self
            .lineage_inspection
            .as_ref()
            .map(|inspection| inspection.selected_session_id.clone())
        else {
            return;
        };
        let Some(snapshot) = self.lineage_snapshot.as_ref() else {
            return;
        };
        let Some(next_session_id) =
            lineage_sibling_session_id(snapshot, &selected_session_id, offset)
        else {
            chat_view.set_status(Some("This subagent has no sibling Session".to_string()));
            return;
        };
        self.inspect_lineage_session(&next_session_id, chat_view, rt_handle);
    }

    fn cancel_inspected_lineage_session(
        &mut self,
        chat_view: &mut ChatView,
        rt_handle: &tokio::runtime::Handle,
    ) {
        let (Some(snapshot), Some(inspection)) = (
            self.lineage_snapshot.as_ref(),
            self.lineage_inspection.as_ref(),
        ) else {
            return;
        };
        let agent = self.agent.clone();
        let root_session_id = snapshot.root_session_id.clone();
        let session_id = inspection.selected_session_id.clone();
        let result = tokio::task::block_in_place(|| {
            rt_handle.block_on(agent.cancel_lineage_session(&root_session_id, &session_id))
        });
        match result {
            Ok(result) if result.requested => chat_view.set_status(Some(format!(
                "Interrupt requested for subagent Session {session_id}"
            ))),
            Ok(_) => chat_view.set_status(Some("The subagent has no active turn".to_string())),
            Err(error) => chat_view.set_status(Some(format!(
                "Could not interrupt subagent Session: {error}"
            ))),
        }
    }
}

fn is_buffered_lineage_event(event: &AgenticEvent) -> bool {
    matches!(
        event,
        AgenticEvent::DialogTurnStarted { .. }
            | AgenticEvent::DialogTurnCompleted { .. }
            | AgenticEvent::DialogTurnFailed { .. }
            | AgenticEvent::DialogTurnCancelled { .. }
            | AgenticEvent::TextChunk { .. }
            | AgenticEvent::ThinkingChunk { .. }
            | AgenticEvent::ToolEvent { .. }
            | AgenticEvent::UserSteeringInjected { .. }
            | AgenticEvent::ContextCompressionStarted { .. }
            | AgenticEvent::ContextCompressionCompleted { .. }
            | AgenticEvent::ContextCompressionFailed { .. }
            | AgenticEvent::TokenUsageUpdated { .. }
    )
}

fn is_terminal_lineage_event(event: &AgenticEvent, turn_id: &str) -> bool {
    matches!(
        event,
        AgenticEvent::DialogTurnCompleted {
            turn_id: event_turn_id,
            ..
        } | AgenticEvent::DialogTurnFailed {
            turn_id: event_turn_id,
            ..
        } | AgenticEvent::DialogTurnCancelled {
            turn_id: event_turn_id,
            ..
        } if event_turn_id == turn_id
    )
}

fn lineage_settlement_from_event(
    existing: Option<&LineageSettlement>,
    observed_turn: bool,
    event: &AgenticEvent,
) -> Option<LineageSettlement> {
    let turn_id = event.turn_id()?;
    if !observed_turn
        || !is_terminal_lineage_event(event, turn_id)
        || existing.is_some_and(|settlement| settlement.turn_id == turn_id)
    {
        return None;
    }
    Some(LineageSettlement {
        turn_id: turn_id.to_string(),
        preserve_live_terminal: matches!(
            event,
            AgenticEvent::DialogTurnFailed { .. } | AgenticEvent::DialogTurnCancelled { .. }
        ),
    })
}

fn clear_lineage_settlement_for_new_turn(
    inspection: Option<&mut AgentSessionInspection>,
    session_id: &str,
) {
    let Some(inspection) =
        inspection.filter(|inspection| inspection.selected_session_id == session_id)
    else {
        return;
    };
    inspection.settling_turn_ids.clear();
    inspection.preserve_live_terminal = false;
}

fn lineage_terminal_reconciliation_pending(
    inspection: Option<&AgentSessionInspection>,
    session_id: &str,
) -> bool {
    inspection.is_some_and(|inspection| {
        inspection.selected_session_id == session_id
            && (!inspection.settling_turn_ids.is_empty() || inspection.preserve_live_terminal)
    })
}

fn provisional_lineage_replay_turn<'a>(
    settlement_turn_id: Option<&'a str>,
    has_terminal_event: bool,
    active_turn_id: Option<&'a str>,
) -> Option<&'a str> {
    if has_terminal_event {
        settlement_turn_id
    } else {
        active_turn_id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineageRefreshAction {
    RetrySettlement,
    PreserveLiveTerminal,
    ReplaceFromRuntime,
}

fn lineage_refresh_action(
    settling_turn_ids: &BTreeSet<String>,
    active_turn_id: Option<&str>,
    preserve_live_terminal: bool,
) -> LineageRefreshAction {
    if active_turn_id.is_some_and(|turn_id| settling_turn_ids.contains(turn_id)) {
        LineageRefreshAction::RetrySettlement
    } else if preserve_live_terminal {
        LineageRefreshAction::PreserveLiveTerminal
    } else {
        LineageRefreshAction::ReplaceFromRuntime
    }
}

fn push_bounded_lineage_event(
    buffer: &mut VecDeque<BufferedLineageEvent>,
    encoded_bytes: &mut usize,
    event: &AgenticEvent,
    max_bytes: usize,
    max_events: usize,
) {
    let Ok(serialized) = serde_json::to_vec(event) else {
        return;
    };
    let event_bytes = serialized.len();
    if event_bytes > max_bytes || max_events == 0 {
        return;
    }
    while !buffer.is_empty()
        && (buffer.len() >= max_events || encoded_bytes.saturating_add(event_bytes) > max_bytes)
    {
        if let Some(removed) = buffer.pop_front() {
            *encoded_bytes = encoded_bytes.saturating_sub(removed.encoded_bytes);
        }
    }
    buffer.push_back(BufferedLineageEvent {
        event: event.clone(),
        encoded_bytes: event_bytes,
    });
    *encoded_bytes = encoded_bytes.saturating_add(event_bytes);
}

fn build_lineage_chat_state(
    entry: &AgentSessionLineageEntry,
    mut inspection: AgentSessionLineageInspection,
    replay_events: &[AgenticEvent],
) -> ChatState {
    let active_turn_id = inspection.active_turn_id.clone();
    let observed_start = active_turn_id.as_deref().is_some_and(|turn_id| {
        replay_events.iter().any(|event| {
            matches!(
                event,
                AgenticEvent::DialogTurnStarted {
                    turn_id: event_turn_id,
                    ..
                } if event_turn_id == turn_id
            )
        })
    });
    if observed_start {
        inspection
            .transcript
            .messages
            .retain(|message| message.turn_id.as_deref() != active_turn_id.as_deref());
    }
    let mut state = ChatState::from_session_transcript(
        entry.session_id.clone(),
        entry.session_name.clone(),
        entry.agent_type.clone(),
        entry.workspace_path.clone(),
        &inspection.transcript,
    );
    state.reconcile_transcript_turn_events(active_turn_id.as_deref());
    if let Some(turn_id) = active_turn_id.as_deref() {
        if !observed_start {
            state.resume_transcript_turn(turn_id);
        }
        for event in replay_events {
            project_transcript_event(&mut state, event, false);
        }
    }
    state
}

fn merge_live_terminal_into_authoritative(
    authoritative: &mut ChatState,
    live: &ChatState,
    replayed: Option<&ChatState>,
    terminal_turn_id: &str,
) {
    let live_terminal_messages = live
        .messages
        .iter()
        .filter(|message| message.turn_id.as_deref() == Some(terminal_turn_id))
        .cloned()
        .collect::<Vec<_>>();
    let replayed_terminal_messages = replayed
        .into_iter()
        .flat_map(|state| state.messages.iter())
        .filter(|message| message.turn_id.as_deref() == Some(terminal_turn_id))
        .cloned()
        .collect::<Vec<_>>();
    let has_terminal_marker = |messages: &[crate::chat_state::ChatMessage]| {
        messages.iter().any(|message| {
            message.flow_items.iter().any(|item| {
                matches!(
                    item,
                    crate::chat_state::FlowItem::Text { content, .. }
                        if content == "[Cancelled]" || content.starts_with("[Error: ")
                )
            })
        })
    };
    let projected_terminal_messages =
        if has_terminal_marker(&live_terminal_messages) || replayed_terminal_messages.is_empty() {
            &live_terminal_messages
        } else {
            &replayed_terminal_messages
        };
    let authoritative_terminal_messages = authoritative
        .messages
        .iter()
        .filter(|message| message.turn_id.as_deref() == Some(terminal_turn_id))
        .cloned()
        .collect::<Vec<_>>();
    let projected_has_user = projected_terminal_messages
        .iter()
        .any(|message| message.role == crate::chat_state::MessageRole::User);
    let projected_has_assistant = projected_terminal_messages
        .iter()
        .any(|message| message.role == crate::chat_state::MessageRole::Assistant);
    let mut replacement = Vec::new();
    if !projected_has_user {
        replacement.extend(
            authoritative_terminal_messages
                .iter()
                .filter(|message| message.role == crate::chat_state::MessageRole::User)
                .cloned(),
        );
    }
    replacement.extend(projected_terminal_messages.iter().cloned());
    if !projected_has_assistant {
        replacement.extend(
            authoritative_terminal_messages
                .iter()
                .filter(|message| message.role == crate::chat_state::MessageRole::Assistant)
                .cloned(),
        );
    }

    let insertion_index = authoritative
        .messages
        .iter()
        .position(|message| message.turn_id.as_deref() == Some(terminal_turn_id))
        .or_else(|| {
            authoritative.current_turn_id().and_then(|active_turn_id| {
                authoritative
                    .messages
                    .iter()
                    .position(|message| message.turn_id.as_deref() == Some(active_turn_id))
            })
        })
        .unwrap_or(authoritative.messages.len());
    let mut merged = Vec::with_capacity(authoritative.messages.len() + replacement.len());
    let mut inserted = false;
    for (index, message) in authoritative.messages.drain(..).enumerate() {
        if !inserted && index == insertion_index {
            merged.append(&mut replacement);
            inserted = true;
        }
        if message.turn_id.as_deref() != Some(terminal_turn_id) {
            merged.push(message);
        }
    }
    if !inserted {
        merged.append(&mut replacement);
    }
    authoritative.messages = merged;
    authoritative.metadata.message_count = authoritative.messages.len();
}

fn update_lineage_active_turn(snapshot: &mut AgentSessionLineageSnapshot, event: &AgenticEvent) {
    let Some(session_id) = event.session_id() else {
        return;
    };
    let Some(entry) = snapshot
        .sessions
        .iter_mut()
        .find(|entry| entry.session_id == session_id)
    else {
        return;
    };

    match event {
        AgenticEvent::DialogTurnStarted { turn_id, .. } => {
            entry.active_turn_id = Some(turn_id.clone());
        }
        AgenticEvent::DialogTurnCompleted { turn_id, .. }
        | AgenticEvent::DialogTurnFailed { turn_id, .. }
        | AgenticEvent::DialogTurnCancelled { turn_id, .. }
            if entry.active_turn_id.as_deref() == Some(turn_id.as_str()) =>
        {
            entry.active_turn_id = None;
        }
        _ => {}
    }
}

fn lineage_parent_session_id(
    snapshot: &AgentSessionLineageSnapshot,
    selected_session_id: &str,
) -> Option<String> {
    snapshot
        .sessions
        .iter()
        .find(|entry| entry.session_id == selected_session_id)
        .and_then(|entry| entry.parent_session_id.clone())
}

fn lineage_sibling_session_id(
    snapshot: &AgentSessionLineageSnapshot,
    selected_session_id: &str,
    offset: isize,
) -> Option<String> {
    let parent = snapshot
        .sessions
        .iter()
        .find(|entry| entry.session_id == selected_session_id)?
        .parent_session_id
        .as_deref();
    let siblings = snapshot
        .sessions
        .iter()
        .filter(|entry| {
            entry.session_id != snapshot.root_session_id
                && entry.parent_session_id.as_deref() == parent
        })
        .map(|entry| entry.session_id.as_str())
        .collect::<Vec<_>>();
    if siblings.len() < 2 {
        return None;
    }
    let index = siblings
        .iter()
        .position(|session_id| *session_id == selected_session_id)?;
    let next = (index as isize + offset).rem_euclid(siblings.len() as isize) as usize;
    Some(siblings[next].to_string())
}

#[cfg(test)]
mod session_lineage_tests {
    use bitfun_agent_runtime::sdk::{
        AgentSessionLifecycleStatus, AgentSessionLineageEntry, AgentSessionLineageInspection,
        AgentSessionLineageSnapshot, SessionTranscript, TranscriptContent, TranscriptMessage,
    };
    use bitfun_events::AgenticEvent;

    use crate::chat_state::{FlowItem, MessageRole};
    use std::collections::{BTreeSet, VecDeque};

    use super::{
        build_lineage_chat_state, clear_lineage_settlement_for_new_turn, lineage_parent_session_id,
        lineage_refresh_action, lineage_settlement_from_event, lineage_sibling_session_id,
        lineage_terminal_reconciliation_pending, merge_live_terminal_into_authoritative,
        project_transcript_event, provisional_lineage_replay_turn, push_bounded_lineage_event,
        update_lineage_active_turn, AgentSessionInspection, BufferedLineageEvent,
        LineageRefreshAction, LineageSettlement,
    };

    fn entry(id: &str, parent: Option<&str>) -> AgentSessionLineageEntry {
        AgentSessionLineageEntry {
            session_id: id.to_string(),
            session_name: id.to_string(),
            agent_type: "explore".to_string(),
            created_at_ms: 1,
            status: AgentSessionLifecycleStatus::Completed,
            active_turn_id: None,
            parent_session_id: parent.map(str::to_string),
            parent_tool_call_id: None,
            subagent_type: Some("explore".to_string()),
            workspace_path: None,
            remote_connection_id: None,
            remote_ssh_host: None,
            unread_completion: None,
            needs_user_attention: None,
        }
    }

    fn snapshot() -> AgentSessionLineageSnapshot {
        AgentSessionLineageSnapshot {
            root_session_id: "root".to_string(),
            sessions: vec![
                entry("root", None),
                entry("first", Some("root")),
                entry("nested", Some("first")),
                entry("second", Some("root")),
            ],
        }
    }

    #[test]
    fn parent_navigation_returns_the_authoritative_parent() {
        let snapshot = snapshot();

        assert_eq!(
            lineage_parent_session_id(&snapshot, "nested").as_deref(),
            Some("first")
        );
        assert_eq!(
            lineage_parent_session_id(&snapshot, "first").as_deref(),
            Some("root")
        );
    }

    #[test]
    fn sibling_navigation_wraps_without_treating_root_as_a_child() {
        let snapshot = snapshot();

        assert_eq!(
            lineage_sibling_session_id(&snapshot, "first", 1).as_deref(),
            Some("second")
        );
        assert_eq!(
            lineage_sibling_session_id(&snapshot, "first", -1).as_deref(),
            Some("second")
        );
        assert_eq!(lineage_sibling_session_id(&snapshot, "nested", 1), None);
    }

    #[test]
    fn background_child_events_refresh_the_existing_lineage_snapshot() {
        let mut snapshot = snapshot();
        update_lineage_active_turn(
            &mut snapshot,
            &AgenticEvent::DialogTurnStarted {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
                turn_index: 1,
                user_input: "continue".to_string(),
                original_user_input: None,
                user_message_metadata: None,
            },
        );
        assert_eq!(
            snapshot.sessions[1].active_turn_id.as_deref(),
            Some("turn-live")
        );

        update_lineage_active_turn(
            &mut snapshot,
            &AgenticEvent::DialogTurnCancelled {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
            },
        );
        assert_eq!(snapshot.sessions[1].active_turn_id, None);
    }

    #[test]
    fn cached_start_rebuilds_the_active_turn_without_duplicate_messages() {
        let entry = entry("first", Some("root"));
        let inspection = AgentSessionLineageInspection {
            transcript: SessionTranscript {
                session_id: "first".to_string(),
                messages: vec![
                    TranscriptMessage {
                        id: Some("persisted-user".to_string()),
                        role: "user".to_string(),
                        turn_id: Some("turn-live".to_string()),
                        timestamp_ms: Some(1),
                        content: TranscriptContent::Text("continue".to_string()),
                    },
                    TranscriptMessage {
                        id: Some("persisted-assistant".to_string()),
                        role: "assistant".to_string(),
                        turn_id: Some("turn-live".to_string()),
                        timestamp_ms: Some(2),
                        content: TranscriptContent::Text("stale output".to_string()),
                    },
                ],
            },
            active_turn_id: Some("turn-live".to_string()),
        };
        let replay_events = vec![
            AgenticEvent::DialogTurnStarted {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
                turn_index: 1,
                user_input: "continue".to_string(),
                original_user_input: None,
                user_message_metadata: None,
            },
            AgenticEvent::TextChunk {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
                round_id: "round-live".to_string(),
                attempt_id: None,
                attempt_index: None,
                text: "hello".to_string(),
            },
        ];

        let state = build_lineage_chat_state(&entry, inspection, &replay_events);

        assert_eq!(state.current_turn_id(), Some("turn-live"));
        assert_eq!(
            state
                .messages
                .iter()
                .filter(|message| message.role == MessageRole::User)
                .count(),
            1
        );
        assert!(state.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "hello"),
                )
        }));
        assert!(!state.messages.iter().any(|message| {
            message.flow_items.iter().any(
                |item| matches!(item, FlowItem::Text { content, .. } if content == "stale output"),
            )
        }));
    }

    #[test]
    fn active_child_replays_cached_tail_and_keeps_streaming_live() {
        let entry = entry("first", Some("root"));
        let inspection = AgentSessionLineageInspection {
            transcript: SessionTranscript {
                session_id: "first".to_string(),
                messages: vec![TranscriptMessage {
                    id: Some("persisted-user".to_string()),
                    role: "user".to_string(),
                    turn_id: Some("turn-live".to_string()),
                    timestamp_ms: Some(1),
                    content: TranscriptContent::Text("continue".to_string()),
                }],
            },
            active_turn_id: Some("turn-live".to_string()),
        };
        let first_chunk = AgenticEvent::TextChunk {
            session_id: "first".to_string(),
            turn_id: "turn-live".to_string(),
            round_id: "round-live".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: "hello".to_string(),
        };
        let mut state = build_lineage_chat_state(&entry, inspection, &[first_chunk]);
        let second_chunk = AgenticEvent::TextChunk {
            session_id: "first".to_string(),
            turn_id: "turn-live".to_string(),
            round_id: "round-live".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: " world".to_string(),
        };

        let outcome = project_transcript_event(&mut state, &second_chunk, false);

        assert!(outcome.changed);
        assert!(state.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.flow_items.iter().any(|item| {
                    matches!(item, FlowItem::Text { content, .. } if content == "hello world")
                })
        }));
    }

    #[test]
    fn reopened_cancelled_child_replays_the_terminal_event_and_partial_output() {
        let entry = entry("first", Some("root"));
        let inspection = AgentSessionLineageInspection {
            transcript: SessionTranscript {
                session_id: "first".to_string(),
                messages: vec![TranscriptMessage {
                    id: Some("persisted-user".to_string()),
                    role: "user".to_string(),
                    turn_id: Some("turn-live".to_string()),
                    timestamp_ms: Some(1),
                    content: TranscriptContent::Text("continue".to_string()),
                }],
            },
            active_turn_id: Some("turn-live".to_string()),
        };
        let replay_events = vec![
            AgenticEvent::TextChunk {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
                round_id: "round-live".to_string(),
                attempt_id: None,
                attempt_index: None,
                text: "partial".to_string(),
            },
            AgenticEvent::DialogTurnCancelled {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
            },
        ];

        let state = build_lineage_chat_state(&entry, inspection, &replay_events);

        assert_eq!(state.current_turn_id(), None);
        assert!(!state.is_processing);
        assert!(state.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "partial"),
                )
        }));
    }

    #[test]
    fn stale_terminal_cannot_replace_the_observed_active_turn() {
        let terminal = AgenticEvent::DialogTurnCancelled {
            session_id: "first".to_string(),
            turn_id: "turn-old".to_string(),
        };

        assert!(lineage_settlement_from_event(None, false, &terminal).is_none());
        let settlement = lineage_settlement_from_event(None, true, &terminal).unwrap();
        assert_eq!(settlement.turn_id, "turn-old");
        assert!(settlement.preserve_live_terminal);
    }

    #[test]
    fn duplicate_terminal_for_the_same_turn_is_ignored() {
        let existing = LineageSettlement {
            turn_id: "turn-live".to_string(),
            preserve_live_terminal: true,
        };
        let duplicate = AgenticEvent::DialogTurnCancelled {
            session_id: "first".to_string(),
            turn_id: "turn-live".to_string(),
        };

        assert!(lineage_settlement_from_event(Some(&existing), true, &duplicate).is_none());
    }

    #[test]
    fn new_turn_keeps_terminal_reconciliation_until_authoritative_refresh() {
        let mut inspection = AgentSessionInspection {
            selected_session_id: "first".to_string(),
            chat_state: crate::chat_state::ChatState::new(
                "first".to_string(),
                "first".to_string(),
                "explore".to_string(),
                None,
            ),
            refresh_pending: true,
            refresh_due_at: std::time::Instant::now(),
            refresh_deadline: Some(std::time::Instant::now()),
            settling_turn_ids: BTreeSet::from(["turn-old".to_string()]),
            preserve_live_terminal: true,
        };

        assert!(lineage_terminal_reconciliation_pending(
            Some(&inspection),
            "first"
        ));
        assert!(inspection.refresh_pending);
        assert!(inspection.refresh_deadline.is_some());
        assert_eq!(
            inspection.settling_turn_ids,
            BTreeSet::from(["turn-old".to_string()])
        );
        assert!(inspection.preserve_live_terminal);
        assert_eq!(
            lineage_refresh_action(
                &inspection.settling_turn_ids,
                Some("turn-new"),
                inspection.preserve_live_terminal,
            ),
            LineageRefreshAction::PreserveLiveTerminal
        );

        inspection.settling_turn_ids.clear();
        inspection.preserve_live_terminal = false;
        assert!(!lineage_terminal_reconciliation_pending(
            Some(&inspection),
            "first"
        ));
        clear_lineage_settlement_for_new_turn(Some(&mut inspection), "first");
        assert!(inspection.refresh_pending);
    }

    #[test]
    fn settled_transcript_fills_provisional_history_without_losing_live_terminal() {
        let entry = entry("first", Some("root"));
        let replay_events = vec![
            AgenticEvent::TextChunk {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
                round_id: "round-live".to_string(),
                attempt_id: None,
                attempt_index: None,
                text: "partial".to_string(),
            },
            AgenticEvent::DialogTurnCancelled {
                session_id: "first".to_string(),
                turn_id: "turn-live".to_string(),
            },
        ];
        let live = crate::chat_state::ChatState::new(
            "first".to_string(),
            "first".to_string(),
            "explore".to_string(),
            None,
        );
        let inspection = AgentSessionLineageInspection {
            transcript: SessionTranscript {
                session_id: "first".to_string(),
                messages: vec![TranscriptMessage {
                    id: Some("persisted-user".to_string()),
                    role: "user".to_string(),
                    turn_id: Some("turn-live".to_string()),
                    timestamp_ms: Some(1),
                    content: TranscriptContent::Text("continue".to_string()),
                }],
            },
            active_turn_id: None,
        };
        let mut authoritative = build_lineage_chat_state(&entry, inspection.clone(), &[]);
        let mut replay_inspection = inspection;
        replay_inspection.active_turn_id = Some("turn-live".to_string());
        let replayed = build_lineage_chat_state(&entry, replay_inspection, &replay_events);

        merge_live_terminal_into_authoritative(
            &mut authoritative,
            &live,
            Some(&replayed),
            "turn-live",
        );

        assert!(authoritative.messages.iter().any(|message| {
            message.role == MessageRole::User
                && message.turn_id.as_deref() == Some("turn-live")
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "continue"),
                )
        }));
        assert!(authoritative.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "partial"),
                )
        }));
        assert_eq!(
            authoritative
                .messages
                .iter()
                .flat_map(|message| message.flow_items.iter())
                .filter(|item| {
                    matches!(item, FlowItem::Text { content, .. } if content == "[Cancelled]")
                })
                .count(),
            1
        );
    }

    #[test]
    fn runtime_active_turn_stays_authoritative_while_previous_live_terminal_is_preserved() {
        let entry = entry("first", Some("root"));
        let live_events = vec![
            AgenticEvent::TextChunk {
                session_id: "first".to_string(),
                turn_id: "turn-a".to_string(),
                round_id: "round-a".to_string(),
                attempt_id: None,
                attempt_index: None,
                text: "partial a".to_string(),
            },
            AgenticEvent::DialogTurnCancelled {
                session_id: "first".to_string(),
                turn_id: "turn-a".to_string(),
            },
        ];
        let mut live = build_lineage_chat_state(
            &entry,
            AgentSessionLineageInspection {
                transcript: SessionTranscript {
                    session_id: "first".to_string(),
                    messages: Vec::new(),
                },
                active_turn_id: Some("turn-a".to_string()),
            },
            &live_events,
        );
        let next_start = AgenticEvent::DialogTurnStarted {
            session_id: "first".to_string(),
            turn_id: "turn-b".to_string(),
            turn_index: 1,
            user_input: "follow up".to_string(),
            original_user_input: None,
            user_message_metadata: None,
        };
        assert!(project_transcript_event(&mut live, &next_start, false).changed);
        let mut authoritative = build_lineage_chat_state(
            &entry,
            AgentSessionLineageInspection {
                transcript: SessionTranscript {
                    session_id: "first".to_string(),
                    messages: vec![
                        TranscriptMessage {
                            id: Some("user-a".to_string()),
                            role: "user".to_string(),
                            turn_id: Some("turn-a".to_string()),
                            timestamp_ms: Some(1),
                            content: TranscriptContent::Text("first".to_string()),
                        },
                        TranscriptMessage {
                            id: Some("user-b".to_string()),
                            role: "user".to_string(),
                            turn_id: Some("turn-b".to_string()),
                            timestamp_ms: Some(2),
                            content: TranscriptContent::Text("follow up".to_string()),
                        },
                    ],
                },
                active_turn_id: Some("turn-b".to_string()),
            },
            &[],
        );

        merge_live_terminal_into_authoritative(&mut authoritative, &live, None, "turn-a");

        assert_eq!(authoritative.current_turn_id(), Some("turn-b"));
        assert_eq!(
            authoritative
                .messages
                .iter()
                .filter_map(|message| message.turn_id.as_deref())
                .collect::<Vec<_>>(),
            vec!["turn-a", "turn-a", "turn-b", "turn-b"]
        );
        assert!(authoritative.messages.iter().any(|message| {
            message.role == MessageRole::User
                && message.turn_id.as_deref() == Some("turn-a")
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "first"),
                )
        }));
        assert!(authoritative.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.turn_id.as_deref() == Some("turn-a")
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "partial a"),
                )
        }));
        let delayed_start = project_transcript_event(
            &mut authoritative,
            &AgenticEvent::DialogTurnStarted {
                session_id: "first".to_string(),
                turn_id: "turn-b".to_string(),
                turn_index: 1,
                user_input: "follow up".to_string(),
                original_user_input: None,
                user_message_metadata: None,
            },
            false,
        );
        assert!(!delayed_start.changed);
        assert_eq!(
            authoritative
                .messages
                .iter()
                .filter(|message| {
                    message.role == MessageRole::User
                        && message.turn_id.as_deref() == Some("turn-b")
                })
                .count(),
            1
        );
        let chunk = project_transcript_event(
            &mut authoritative,
            &AgenticEvent::TextChunk {
                session_id: "first".to_string(),
                turn_id: "turn-b".to_string(),
                round_id: "round-b".to_string(),
                attempt_id: None,
                attempt_index: None,
                text: "live b".to_string(),
            },
            false,
        );
        assert!(chunk.changed);
        assert!(authoritative.messages.iter().any(|message| {
            message.role == MessageRole::Assistant
                && message.turn_id.as_deref() == Some("turn-b")
                && message.flow_items.iter().any(
                    |item| matches!(item, FlowItem::Text { content, .. } if content == "live b"),
                )
        }));
    }

    #[test]
    fn provisional_replay_uses_the_lineage_active_turn_before_terminal_arrives() {
        assert_eq!(
            provisional_lineage_replay_turn(None, false, Some("turn-live")),
            Some("turn-live")
        );
    }

    #[test]
    fn lineage_event_buffer_is_bounded_by_serialized_bytes_and_count() {
        let event = |text: &str| AgenticEvent::TextChunk {
            session_id: "first".to_string(),
            turn_id: "turn-live".to_string(),
            round_id: "round-live".to_string(),
            attempt_id: None,
            attempt_index: None,
            text: text.to_string(),
        };
        let first = event("first");
        let second = event("second");
        let max_bytes = serde_json::to_vec(&second).unwrap().len();
        let mut buffer: VecDeque<BufferedLineageEvent> = VecDeque::new();
        let mut encoded_bytes = 0;

        push_bounded_lineage_event(&mut buffer, &mut encoded_bytes, &first, max_bytes, 1);
        push_bounded_lineage_event(&mut buffer, &mut encoded_bytes, &second, max_bytes, 1);

        assert_eq!(buffer.len(), 1);
        assert!(matches!(
            &buffer[0].event,
            AgenticEvent::TextChunk { text, .. } if text == "second"
        ));
        assert!(encoded_bytes <= max_bytes);

        let oversized = event(&"x".repeat(max_bytes));
        push_bounded_lineage_event(&mut buffer, &mut encoded_bytes, &oversized, max_bytes, 1);
        assert_eq!(buffer.len(), 1);
    }

    #[test]
    fn terminal_reconciliation_retries_while_the_settling_turn_is_still_active() {
        let settling = BTreeSet::from(["turn-terminal".to_string()]);

        assert_eq!(
            lineage_refresh_action(&settling, Some("turn-terminal"), true),
            LineageRefreshAction::RetrySettlement
        );
        assert_eq!(
            lineage_refresh_action(&settling, None, true),
            LineageRefreshAction::PreserveLiveTerminal
        );
        assert_eq!(
            lineage_refresh_action(&settling, Some("turn-new"), true),
            LineageRefreshAction::PreserveLiveTerminal
        );
        assert_eq!(
            lineage_refresh_action(&settling, None, false),
            LineageRefreshAction::ReplaceFromRuntime
        );
    }

    #[test]
    fn delayed_start_for_a_persisted_turn_is_ignored_after_inspection() {
        let entry = entry("first", Some("root"));
        let inspection = AgentSessionLineageInspection {
            transcript: SessionTranscript {
                session_id: "first".to_string(),
                messages: vec![
                    TranscriptMessage {
                        id: Some("user".to_string()),
                        role: "user".to_string(),
                        turn_id: Some("turn-done".to_string()),
                        timestamp_ms: Some(1),
                        content: TranscriptContent::Text("question".to_string()),
                    },
                    TranscriptMessage {
                        id: Some("assistant".to_string()),
                        role: "assistant".to_string(),
                        turn_id: Some("turn-done".to_string()),
                        timestamp_ms: Some(2),
                        content: TranscriptContent::Text("answer".to_string()),
                    },
                ],
            },
            active_turn_id: None,
        };
        let mut state = build_lineage_chat_state(&entry, inspection, &[]);
        let before = state.messages.len();

        let outcome = project_transcript_event(
            &mut state,
            &AgenticEvent::DialogTurnStarted {
                session_id: "first".to_string(),
                turn_id: "turn-done".to_string(),
                turn_index: 0,
                user_input: "question".to_string(),
                original_user_input: None,
                user_message_metadata: None,
            },
            false,
        );

        assert!(!outcome.changed);
        assert_eq!(state.messages.len(), before);
    }
}
