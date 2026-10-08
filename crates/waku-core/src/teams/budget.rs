//! Durable reservations before billable calls. No tokenizer estimates or model-supplied receipts.
use super::*;
use serde::{Deserialize, Serialize};

/// Supplied by the trusted adapter from a verified model context limit and the
/// exact requested completion bound. Includes system messages, tools and reasoning.
#[derive(Clone, Debug)]
pub struct ModelCallAllowance {
    pub model: String,
    pub max_input_tokens: u64,
    pub max_output_tokens: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTokenUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
/// Opaque receipt capability. A stopped/expired worker may finish accounting for
/// its already-reserved request, but cannot reserve another billable request.
#[derive(Clone, Debug)]
pub struct ModelCallReservation {
    id: Uuid,
    run_id: Uuid,
    work_id: Uuid,
    max_input_tokens: u64,
    max_output_tokens: u64,
}
impl TeamCoordinator {
    /// A confirmed remote worker can deliver usage after Stop/restart without
    /// granting execution. Model/harness identity stays pinned to its call.
    pub(crate) fn record_remote_model_usage(
        &self,
        id: Uuid,
        provider: waku_protocol::model::ProviderKind,
        model: &str,
        usage: ModelTokenUsage,
        now: u64,
    ) -> Result<()> {
        let db = self.connect()?;
        let (run,work,pinned,input,output,body):(String,String,String,u64,u64,String)=db.query_row(
            "SELECT c.run_id,c.work_id,c.model,b.input_cap,b.output_cap,r.receipt FROM ion_team_model_calls c JOIN ion_team_call_budgets b ON b.call_id=c.id JOIN ion_team_remote_calls r ON r.call_id=c.id WHERE c.id=? AND r.state='SUCCESS' AND r.receipt IS NOT NULL",
            [id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
        let evidence: RemoteUsageReceipt = serde_json::from_str(&body)?;
        ensure!(
            evidence.version == 1
                && evidence.request_id == id
                && evidence.provider == provider
                && evidence.model == model
                && evidence.usage == usage,
            "Remote usage differs from the saved receipt"
        );
        ensure!(
            evidence
                .estimated_cost_usd_micros
                .is_none_or(|micros| evidence.provider
                    == waku_protocol::model::ProviderKind::Claude
                    && micros <= 1_000_000_000_000),
            "Remote SDK estimate contract changed"
        );
        ensure!(pinned == model, "Remote usage model changed");
        let snapshot = load(&db, Uuid::parse_str(&run)?)?;
        let assignment = snapshot
            .work
            .iter()
            .find(|w| w.id.to_string() == work)
            .context("Remote assignment missing")?;
        ensure!(
            snapshot
                .profiles
                .iter()
                .any(|p| p.id == assignment.agent_id && p.provider == provider),
            "Remote usage harness changed"
        );
        self.record_model_usage(
            &ModelCallReservation {
                id,
                run_id: Uuid::parse_str(&run)?,
                work_id: Uuid::parse_str(&work)?,
                max_input_tokens: input,
                max_output_tokens: output,
            },
            usage,
            now,
        )
    }

    pub(crate) fn record_model_cost(
        &self,
        call: &ModelCallReservation,
        cost: &waku_protocol::usage_ledger::UsageCost,
        now: u64,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let present:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM ion_team_model_calls WHERE id=? AND run_id=? AND work_id=?)",params![call.id.to_string(),call.run_id.to_string(),call.work_id.to_string()],|r|r.get(0))?;
        ensure!(present, "Model cost reservation identity changed");
        crate::usage_ledger::settle_cost(
            &tx,
            waku_protocol::usage_ledger::UsageLedgerKind::ModelCall,
            call.id,
            cost,
            now,
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn reserve_model_call(
        &self,
        lease: &AssignmentLease,
        step: u16,
        allowance: ModelCallAllowance,
        now: u64,
    ) -> Result<ModelCallReservation> {
        ensure!(step > 0, "Model call step must be positive");
        ensure!(
            (1..=32_000_000).contains(&allowance.max_input_tokens)
                && (1..=32_000_000).contains(&allowance.max_output_tokens),
            "Invalid model token allowance"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = load(&tx, lease.run_id)?;
        active(&run, now)?;
        super::workspace::require_settled_effects(&tx, run.id)?;
        let work = checked_lease(&run, lease, now, false)?;
        let model = run
            .profiles
            .iter()
            .find(|p| p.id == work.agent_id)
            .and_then(|p| p.model.as_deref())
            .context("Pinned model is required")?;
        ensure!(
            model == allowance.model,
            "Token allowance must match the pinned model"
        );
        ensure!(
            run.usage.model_calls < u32::from(run.limits.max_model_calls),
            "Team model-call budget reached"
        );
        ensure!(
            run.usage.allowance_violations == 0,
            "Provider exceeded a prior token allowance; stop and review this run"
        );
        if let Some(limit) = run.limits.max_tokens {
            ensure!(
                run.usage.unmetered_calls == 0,
                "Historical calls lack token allowances; budget cannot be verified"
            );
            let committed = run.usage.consumed_tokens + run.usage.reserved_tokens;
            let requested = allowance.max_input_tokens + allowance.max_output_tokens;
            ensure!(
                committed + requested <= limit,
                "Team token budget reached: {committed} committed, {requested} required, {limit} limit"
            );
        }
        let id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO ion_team_model_calls VALUES(?,?,?,?,?,?,?,?)",
            params![
                id.to_string(),
                run.id.to_string(),
                work.id.to_string(),
                lease.attempt_id.to_string(),
                lease.generation,
                step,
                model,
                now
            ],
        )
        .context("Model call was already reserved; do not replay a provider request")?;
        tx.execute(
            "INSERT INTO ion_team_call_budgets(call_id,input_cap,output_cap) VALUES(?,?,?)",
            params![
                id.to_string(),
                allowance.max_input_tokens,
                allowance.max_output_tokens
            ],
        )?;
        audit(
            &tx,
            run.id,
            Some(work.id),
            "model_call",
            &format!(
                "Reserved call {id} for pinned model {model}: {} input + {} output tokens",
                allowance.max_input_tokens, allowance.max_output_tokens
            ),
            now,
        )?;
        bump(&tx, run.id)?;
        tx.commit()?;
        Ok(ModelCallReservation {
            id,
            run_id: run.id,
            work_id: work.id,
            max_input_tokens: allowance.max_input_tokens,
            max_output_tokens: allowance.max_output_tokens,
        })
    }

    /// Only a complete, validated provider stream supplies this receipt. Missing
    /// usage, transport failure, cancellation and restart keep the full allowance.
    pub fn record_model_usage(
        &self,
        call: &ModelCallReservation,
        receipt: ModelTokenUsage,
        now: u64,
    ) -> Result<()> {
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let exceeded = settle_usage(&tx, call, receipt, now)?;
        tx.commit()?;
        ensure!(
            !exceeded,
            "Provider exceeded its token allowance; stop and review the run"
        );
        Ok(())
    }
}

pub(super) fn usage(db: &Connection, run: Uuid) -> Result<TeamTokenUsage> {
    Ok(db.query_row("SELECT count(*),COALESCE(sum(CASE WHEN b.input_tokens IS NOT NULL THEN b.input_tokens+b.output_tokens ELSE 0 END),0),COALESCE(sum(CASE WHEN b.input_tokens IS NULL THEN b.input_cap+b.output_cap ELSE 0 END),0),COALESCE(sum(CASE WHEN b.input_tokens IS NULL THEN 1 ELSE 0 END),0),COALESCE(sum(CASE WHEN b.call_id IS NULL THEN 1 ELSE 0 END),0),COALESCE(sum(CASE WHEN b.input_tokens>b.input_cap OR b.output_tokens>b.output_cap THEN 1 ELSE 0 END),0) FROM ion_team_model_calls c LEFT JOIN ion_team_call_budgets b ON b.call_id=c.id WHERE c.run_id=?",
        [run.to_string()],|r|Ok(TeamTokenUsage{model_calls:r.get(0)?,consumed_tokens:r.get(1)?,reserved_tokens:r.get(2)?,unknown_calls:r.get(3)?,unmetered_calls:r.get(4)?,allowance_violations:r.get(5)?}))?)
}

/// Shared by ordinary settlement and the remote process receipt transaction.
fn settle_usage(
    db: &Transaction<'_>,
    call: &ModelCallReservation,
    receipt: ModelTokenUsage,
    now: u64,
) -> Result<bool> {
    ensure!(
        receipt.input_tokens <= 1_000_000_000_000 && receipt.output_tokens <= 1_000_000_000_000,
        "Invalid provider token receipt"
    );
    let (input_cap,output_cap,input,output):(u64,u64,Option<u64>,Option<u64>) = db.query_row(
            "SELECT b.input_cap,b.output_cap,b.input_tokens,b.output_tokens FROM ion_team_call_budgets b JOIN ion_team_model_calls c ON c.id=b.call_id WHERE c.id=? AND c.run_id=? AND c.work_id=?",
            params![call.id.to_string(),call.run_id.to_string(),call.work_id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
    ensure!(
        input_cap == call.max_input_tokens && output_cap == call.max_output_tokens,
        "Token reservation changed"
    );
    if let (Some(input), Some(output)) = (input, output) {
        ensure!(
            input == receipt.input_tokens && output == receipt.output_tokens,
            "Provider token receipt changed; cannot release another reservation"
        );
        return Ok(input > input_cap || output > output_cap);
    }
    db.execute(
        "UPDATE ion_team_call_budgets SET input_tokens=?,output_tokens=? WHERE call_id=?",
        params![
            receipt.input_tokens,
            receipt.output_tokens,
            call.id.to_string()
        ],
    )?;
    let exceeded = receipt.input_tokens > input_cap || receipt.output_tokens > output_cap;
    audit(
        db,
        call.run_id,
        Some(call.work_id),
        if exceeded {
            "token_allowance_exceeded"
        } else {
            "model_usage"
        },
        &format!(
            "Call {} provider receipt: {} input + {} output tokens",
            call.id, receipt.input_tokens, receipt.output_tokens
        ),
        now,
    )?;
    bump(db, call.run_id)?;
    Ok(exceeded)
}
#[derive(Deserialize)]
struct RemoteUsageReceipt {
    version: u16,
    request_id: Uuid,
    provider: waku_protocol::model::ProviderKind,
    model: String,
    usage: ModelTokenUsage,
    #[serde(default)]
    estimated_cost_usd_micros: Option<u64>,
}
/// A stored process-verified receipt settles usage in the terminal transaction.
pub(super) fn settle_remote_receipt(db: &Transaction<'_>, id: Uuid, now: u64) -> Result<()> {
    let (body,run,work,model,input,output):(String,String,String,String,u64,u64)=db.query_row(
        "SELECT r.receipt,c.run_id,c.work_id,c.model,b.input_cap,b.output_cap FROM ion_team_remote_calls r JOIN ion_team_model_calls c ON c.id=r.call_id JOIN ion_team_call_budgets b ON b.call_id=c.id WHERE r.call_id=? AND r.state='SUCCESS' AND r.receipt IS NOT NULL",
        [id.to_string()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?)))?;
    let receipt: RemoteUsageReceipt = serde_json::from_str(&body)?;
    ensure!(
        receipt.version == 1 && receipt.request_id == id && receipt.model == model,
        "Remote usage identity changed"
    );
    let run_id = Uuid::parse_str(&run)?;
    let work_id = Uuid::parse_str(&work)?;
    let snapshot = load(db, run_id)?;
    let assignment = snapshot
        .work
        .iter()
        .find(|w| w.id == work_id)
        .context("Remote assignment missing")?;
    ensure!(
        snapshot
            .profiles
            .iter()
            .any(|p| p.id == assignment.agent_id && p.provider == receipt.provider),
        "Remote usage harness changed"
    );
    ensure!(
        receipt
            .estimated_cost_usd_micros
            .is_none_or(
                |micros| receipt.provider == waku_protocol::model::ProviderKind::Claude
                    && micros <= 1_000_000_000_000
            ),
        "Remote SDK estimate contract changed"
    );
    let call = ModelCallReservation {
        id,
        run_id,
        work_id,
        max_input_tokens: input,
        max_output_tokens: output,
    };
    settle_usage(db, &call, receipt.usage, now)?;
    if let Some(micros) = receipt.estimated_cost_usd_micros {
        crate::usage_ledger::settle_cost(
            db,
            waku_protocol::usage_ledger::UsageLedgerKind::ModelCall,
            id,
            &crate::usage_ledger::claude_sdk_estimate(micros),
            now,
        )?;
    }
    Ok(())
}
