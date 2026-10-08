//! Bounded source-team work and approval, independent of native wire protocol.
use super::{WorkspaceContext, execute};
use crate::driver::DriverEventSender;
use anyhow::{Result, ensure};
use crossbeam_channel::{Sender, bounded};
use parking_lot::Mutex;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use uuid::Uuid;
use waku_protocol::{
    control::AgentProfile,
    model::{ActivityKind, DriverEvent, PermissionOption},
    teams::{AgentAccess, TeamRunLimits},
};
#[derive(Clone)]
pub(crate) struct Worker {
    state: Arc<State>,
    jobs: Sender<Call>,
}
struct State {
    approvals: Mutex<HashMap<String, Sender<bool>>>,
    closed: AtomicBool,
}
pub(crate) struct Call {
    pub peer: Value,
    pub request_id: String,
    pub name: String,
    pub arguments: Value,
    pub cancelled: Arc<AtomicBool>,
}
impl Worker {
    pub(crate) fn start(
        context: WorkspaceContext,
        events: DriverEventSender,
        deliver: impl Fn(Value, bool, String) + Send + 'static,
    ) -> Result<Self> {
        let state = Arc::new(State {
            approvals: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
        });
        let (jobs, received) = bounded::<Call>(4);
        let worker_state = state.clone();
        thread::Builder::new().name("ion-source-team-tools".into()).spawn(move || {
            while !worker_state.closed.load(Ordering::Acquire) {
                let call=match received.recv_timeout(Duration::from_millis(100)) {
                    Ok(call)=>call,
                    Err(crossbeam_channel::RecvTimeoutError::Timeout)=>continue,
                    Err(_)=>break,
                };
                let activity=|complete, detail| DriverEvent::Activity { id:Some(call.request_id.clone()), kind:ActivityKind::Tool, title:call.name.replace('_'," "), detail, complete };
                let _=events.send(activity(false,None));
                let result=execute(&context,&call.request_id,&call.name,&call.arguments,&call.cancelled, |profile, limits| approve(&worker_state, &call.cancelled, &events, profile, limits));
                let (success,text)=match result {
                    Ok(text)=>(true,text),
                    Err(_)=>(false,"Ion team request did not complete. Check team setup, current definitions, source-task approval and run status. No invocation is automatically retried.".into()),
                };
                let _=events.send(activity(true,Some(text.clone())));
                deliver(call.peer,success,text);
            }
        })?;
        Ok(Self { state, jobs })
    }
    pub(crate) fn is_closed(&self) -> bool {
        self.state.closed.load(Ordering::Acquire)
    }
    pub(crate) fn submit(&self, call: Call) -> std::result::Result<(), Call> {
        if self.is_closed() {
            return Err(call);
        }
        self.jobs.try_send(call).map_err(|e| e.into_inner())
    }
    pub(crate) fn respond(&self, id: &str, option: &str) -> bool {
        if !id.starts_with("ion-team-approval-") {
            return false;
        }
        if let Some(sender) = self.state.approvals.lock().remove(id) {
            let _ = sender.send(option == "allow");
        }
        true
    }
    pub(crate) fn close(&self) {
        self.state.closed.store(true, Ordering::Release);
        self.state.approvals.lock().clear();
    }
}
fn approve(
    state: &State,
    cancelled: &AtomicBool,
    events: &DriverEventSender,
    profile: &AgentProfile,
    limits: &TeamRunLimits,
) -> Result<AgentAccess> {
    ensure!(
        !state.closed.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire),
        "Stopped before approval"
    );
    let id = format!("ion-team-approval-{}", Uuid::new_v4());
    let (sender, received) = bounded(1);
    {
        let mut approvals = state.approvals.lock();
        ensure!(
            !state.closed.load(Ordering::Acquire),
            "Source bridge closed"
        );
        approvals.insert(id.clone(), sender);
    }
    let result = (|| {
        events.send(DriverEvent::Permission {
            request_id:id.clone(), title:format!("Use team agent {}?",profile.name),
            detail:format!("Harness: {}\nModel: {}\nExecution environment: {}\nConfigured access: {}\nRun limits: {}\nEdits use an isolated branch. App actions require their own exact approval.", profile.provider.display_name(),profile.model.as_deref().unwrap_or("default"),profile.execution.environment.label(),serde_json::to_string(&profile.access)?,serde_json::to_string(limits)?),
            options:vec![PermissionOption{id:"allow".into(),label:"Start team".into(),allow:true},PermissionOption{id:"reject".into(),label:"Decline".into(),allow:false}],
        }).map_err(|_|anyhow::anyhow!("Source task closed"))?;
        let deadline = Instant::now() + Duration::from_secs(300);
        loop {
            ensure!(
                !state.closed.load(Ordering::Acquire)
                    && !cancelled.load(Ordering::Acquire)
                    && Instant::now() < deadline,
                "Team approval stopped or expired"
            );
            match received.recv_timeout(Duration::from_millis(100)) {
                Ok(true) => {
                    ensure!(
                        !state.closed.load(Ordering::Acquire) && !cancelled.load(Ordering::Acquire),
                        "Approval canceled"
                    );
                    return Ok(profile.access.clone());
                }
                Ok(false) => anyhow::bail!("Team invocation declined"),
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => continue,
                Err(_) => anyhow::bail!("Approval channel closed"),
            }
        }
    })();
    state.approvals.lock().remove(&id);
    result
}
