//! Native ACP transports share turn fencing without sharing provider dialects.
use super::{
    cursor::{CursorApproval, CursorBridge},
    grok::GrokBridge,
};
use agent_client_protocol::schema::v1::McpServer;
use anyhow::Result;
use serde_json::{Map, Value, json};
#[derive(Clone)]
pub(crate) enum AcpTeamBridge {
    Grok(GrokBridge),
    Cursor(CursorBridge),
}
impl AcpTeamBridge {
    pub(crate) fn bind(&self, session: &str) -> Result<()> {
        match self {
            Self::Grok(b) => b.bind(session),
            Self::Cursor(b) => b.bind(session),
        }
    }
    pub(crate) fn environment(&self) -> Vec<(String, String)> {
        match self {
            Self::Grok(b) => b.environment(),
            Self::Cursor(b) => b.environment(),
        }
    }
    pub(crate) fn mcp_servers(&self) -> Vec<McpServer> {
        match self {
            Self::Grok(b) => b.mcp_servers(),
            Self::Cursor(b) => b.mcp_servers(),
        }
    }
    pub(crate) fn queue_turn(&self) -> u64 {
        match self {
            Self::Grok(b) => b.queue_turn(),
            Self::Cursor(b) => b.queue_turn(),
        }
    }
    pub(crate) fn activate(&self, turn: u64) -> bool {
        match self {
            Self::Grok(b) => b.activate(turn),
            Self::Cursor(b) => b.activate(turn),
        }
    }
    pub(crate) fn active_turn(&self) -> Option<u64> {
        match self {
            Self::Grok(b) => b.active_turn(),
            Self::Cursor(b) => b.active_turn(),
        }
    }
    pub(crate) fn finish_turn(&self, turn: Option<u64>) -> () {
        match self {
            Self::Grok(b) => b.finish_turn(turn),
            Self::Cursor(b) => b.finish_turn(turn),
        }
    }
    pub(crate) fn cancel(&self) -> () {
        match self {
            Self::Grok(b) => b.cancel(),
            Self::Cursor(b) => b.cancel(),
        }
    }
    pub(crate) fn close(&self) -> () {
        match self {
            Self::Grok(b) => b.close(),
            Self::Cursor(b) => b.close(),
        }
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        match self {
            Self::Grok(b) => b.respond(id, option),
            Self::Cursor(b) => b.respond(id, option),
        }
    }
    pub(crate) fn observe(&self, event: &Value) -> () {
        match self {
            Self::Grok(b) => b.observe(event),
            Self::Cursor(b) => b.observe(event),
        }
    }
    pub(crate) fn scrub(&self, value: &mut Value) -> () {
        match self {
            Self::Grok(b) => b.scrub(value),
            Self::Cursor(b) => b.scrub(value),
        }
    }
    pub(crate) fn metadata(&self) -> Map<String, Value> {
        match self {
            Self::Grok(b) => b.metadata(),
            Self::Cursor(_) => Map::new(),
        }
    }
    pub(crate) fn witness(&self, event: &Value) -> Value {
        match self {
            Self::Grok(b) => b.witness(event),
            Self::Cursor(_) => json!({"decision":"deny"}),
        }
    }
    pub(crate) fn permission(&self, request: &Value) -> Result<Option<CursorApproval>> {
        match self {
            Self::Grok(_) => Ok(None),
            Self::Cursor(b) => b.permission(request),
        }
    }
}
