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
        ensure!(
            receipt.input_tokens <= 1_000_000_000_000 && receipt.output_tokens <= 1_000_000_000_000,
            "Invalid provider token receipt"
        );
        let mut db = self.connect()?;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (input_cap,output_cap,input,output):(u64,u64,Option<u64>,Option<u64>) = tx.query_row(
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
            ensure!(
                input <= input_cap && output <= output_cap,
                "Provider exceeded its token allowance; stop and review the run"
            );
            return Ok(());
        }
        tx.execute(
            "UPDATE ion_team_call_budgets SET input_tokens=?,output_tokens=? WHERE call_id=?",
            params![
                receipt.input_tokens,
                receipt.output_tokens,
                call.id.to_string()
            ],
        )?;
        let exceeded = receipt.input_tokens > input_cap || receipt.output_tokens > output_cap;
        audit(
            &tx,
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
        bump(&tx, call.run_id)?;
        tx.commit()?;
        // Persist the true receipt even if a provider violated its advertised cap.
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
