//! Incremental daemon transcript projection. No whole-transcript scan per chunk.
use crate::model::*;
use std::collections::HashMap;

pub(super) struct Projection {
    text: Option<usize>,
    reasoning: Option<(usize, usize)>,
    activities: HashMap<String, (usize, usize)>,
}
impl Projection {
    pub(super) fn new(session: &mut AgentSession) -> Self {
        if session.active_turn_id().is_none() {
            let turn = session.begin_provider_turn();
            if let Some(message) = session
                .messages
                .iter_mut()
                .rev()
                .find(|m| m.role == MessageRole::User && m.turn_id.is_none())
            {
                message.turn_id = Some(turn);
            }
        }
        Self::existing(session)
    }
    pub(super) fn existing(session: &AgentSession) -> Self {
        let activities = session
            .transcript_blocks
            .iter()
            .enumerate()
            .flat_map(|(b, block)| {
                block
                    .activities
                    .iter()
                    .enumerate()
                    .filter_map(move |(a, item)| item.source_id.clone().map(|id| (id, (b, a))))
            })
            .collect();
        Self {
            text: None,
            reasoning: None,
            activities,
        }
    }
    fn close_text(&mut self, session: &mut AgentSession) {
        if let Some(i) = self.text.take() {
            session.messages[i].streaming = false;
        }
    }
    fn close_reasoning(&mut self, session: &mut AgentSession) {
        if let Some((b, a)) = self.reasoning.take() {
            let item = &mut session.transcript_blocks[b].activities[a];
            item.complete = true;
            if let Some(reasoning) = &mut item.reasoning {
                reasoning.finished_at_ms = unix_time() * 1000;
            }
        }
    }
    fn add_activity(
        &mut self,
        session: &mut AgentSession,
        activity: ActivityItem,
    ) -> (usize, usize) {
        let anchor = session.messages.len();
        let turn = session.active_turn_id();
        if session
            .transcript_blocks
            .last()
            .is_none_or(|b| b.after_message != anchor || b.turn_id != turn)
        {
            session.transcript_blocks.push(TranscriptBlock {
                after_message: anchor,
                turn_id: turn,
                activities: vec![],
            });
        }
        let b = session.transcript_blocks.len() - 1;
        let a = session.transcript_blocks[b].activities.len();
        session.transcript_blocks[b].activities.push(activity);
        (b, a)
    }
    pub(super) fn apply(&mut self, session: &mut AgentSession, event: &DriverEvent) {
        match event {
            DriverEvent::TurnStarted => {
                session.mark_active_turn_provider_started();
                session.status = SessionStatus::Working;
                session.team_assignment_status =
                    Some(waku_protocol::teams::AssignmentStatus::Running);
            }
            DriverEvent::TextDelta(delta) => {
                self.close_reasoning(session);
                let i = *self.text.get_or_insert_with(|| {
                    session.push_message(MessageRole::Assistant, "");
                    session.messages.len() - 1
                });
                session.messages[i].content.push_str(delta);
                session.messages[i].streaming = true;
            }
            DriverEvent::ReasoningDelta(delta) => {
                self.close_text(session);
                let slot = if let Some(slot) = self.reasoning {
                    slot
                } else {
                    let slot = self.add_activity(
                        session,
                        ActivityItem::from_reasoning(
                            ReasoningBlock {
                                content: String::new(),
                                started_at_ms: unix_time() * 1000,
                                finished_at_ms: 0,
                            },
                            false,
                        ),
                    );
                    self.reasoning = Some(slot);
                    slot
                };
                session.transcript_blocks[slot.0].activities[slot.1]
                    .reasoning
                    .as_mut()
                    .unwrap()
                    .content
                    .push_str(delta);
            }
            DriverEvent::Activity {
                id,
                kind,
                title,
                detail,
                complete,
            } => {
                self.close_text(session);
                self.close_reasoning(session);
                let item = ActivityItem::new(id.clone(), *kind, title, detail.clone(), *complete);
                if let Some(slot) = id.as_ref().and_then(|id| self.activities.get(id)).copied() {
                    let old = &mut session.transcript_blocks[slot.0].activities[slot.1];
                    let uuid = old.id;
                    *old = item;
                    old.id = uuid;
                } else {
                    let slot = self.add_activity(session, item);
                    if let Some(id) = id {
                        self.activities.insert(id.clone(), slot);
                    }
                }
            }
            DriverEvent::RichActivity(item) => {
                self.close_text(session);
                self.close_reasoning(session);
                if let Some(slot) = item
                    .source_id
                    .as_ref()
                    .and_then(|id| self.activities.get(id))
                    .copied()
                {
                    let old = &mut session.transcript_blocks[slot.0].activities[slot.1];
                    let uuid = old.id;
                    *old = item.clone();
                    old.id = uuid;
                } else {
                    let slot = self.add_activity(session, item.clone());
                    if let Some(id) = &item.source_id {
                        self.activities.insert(id.clone(), slot);
                    }
                }
            }
            DriverEvent::TurnParked => {
                self.close_text(session);
                self.close_reasoning(session);
                session.status = SessionStatus::Background;
                session.team_assignment_status =
                    Some(waku_protocol::teams::AssignmentStatus::Waiting);
            }
            DriverEvent::TurnFinished { success, .. } => {
                self.close_text(session);
                self.close_reasoning(session);
                session.finish_active_turn(if *success {
                    TurnStatus::Completed
                } else {
                    TurnStatus::Failed
                });
                session.team_assignment_status = Some(if *success {
                    waku_protocol::teams::AssignmentStatus::Completed
                } else {
                    waku_protocol::teams::AssignmentStatus::Blocked
                });
                session.status = if *success {
                    SessionStatus::Idle
                } else {
                    SessionStatus::Failed
                };
            }
            DriverEvent::Error(reason) => {
                self.close_text(session);
                self.close_reasoning(session);
                session.push_message(MessageRole::System, reason);
            }
            DriverEvent::UsageUpdated {
                context_tokens,
                context_window,
            } => {
                let usage = session.context_usage.get_or_insert(ContextUsage {
                    tokens: 0,
                    window: None,
                });
                if let Some(tokens) = context_tokens {
                    usage.tokens = *tokens;
                }
                if let Some(window) = context_window {
                    usage.window = Some(*window);
                }
            }
            DriverEvent::Connected { provider_cursor } => {
                session.provider_cursor = provider_cursor.clone()
            }
            _ => {}
        }
        session.updated_at = unix_time();
    }
}
