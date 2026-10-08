use super::*;
use waku_protocol::usage_ledger::*;
fn query() -> UsageLedgerQuery {
    UsageLedgerQuery {
        project_id: None,
        task_id: None,
        agent_id: None,
        provider: None,
        environment: None,
        kind: None,
        model: None,
        since: None,
        after: None,
        limit: 32,
    }
}
fn estimate(currency: &str, micros: u64) -> UsageCost {
    UsageCost {
        amount: UsageMoney {
            currency: currency.into(),
            micros,
        },
        quality: UsageCostQuality::Estimated,
        finality: UsageCostFinality::Provisional,
        origin: "Owner fixture estimate".into(),
    }
}
#[test]
fn global_usage_pins_agent_harness_host_and_retains_exact_receipts_after_stop() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let first = f
        .store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let second = f
        .store
        .reserve_model_call(&root.lease, 2, allowance("model-0", 200, 20), 103)
        .unwrap();
    let page = f.definitions.usage_ledger(query()).unwrap();
    assert_eq!(page.summary.records, 2);
    assert_eq!(page.summary.unknown_calls, 2);
    assert_eq!(page.summary.reserved_tokens, 330);
    assert_eq!(page.summary.unknown_cost_records, 2);
    assert!(page.summary.costs.is_empty());
    let id = page.entries[1].source_id;
    let observed = page.entries[1].observed_at;
    let mut profile = run.profiles[0].clone();
    profile.name = "Edited name".into();
    profile.model = Some("other-model".into());
    profile.provider = ProviderKind::Claude;
    f.definitions
        .save(
            f.definitions.load().unwrap().revision,
            ControlRecord::Agent(profile),
        )
        .unwrap();
    let active = f.store.snapshot(run.id).unwrap();
    f.store.stop(run.id, active.revision, 104).unwrap();
    f.store
        .record_model_usage(
            &first,
            ModelTokenUsage {
                input_tokens: 50,
                output_tokens: 5,
            },
            105,
        )
        .unwrap();
    f.store
        .record_model_cost(&first, &estimate("USD", 12345), 105)
        .unwrap();
    let page = f.definitions.usage_ledger(query()).unwrap();
    assert_eq!(page.summary.input_tokens, 50);
    assert_eq!(page.summary.output_tokens, 5);
    assert_eq!(page.summary.reserved_tokens, 220);
    assert_eq!(page.summary.unknown_calls, 1);
    let entry = page.entries.iter().find(|e| e.source_id == id).unwrap();
    assert_eq!(entry.agent_name.as_deref(), Some("Agent 0"));
    assert_eq!(entry.model.as_deref(), Some("model-0"));
    assert_eq!(entry.provider, Some(ProviderKind::Nebius));
    assert_eq!(entry.environment, Some(ExecutionEnvironment::Local));
    assert_eq!(entry.host, "local");
    assert_eq!(entry.observed_at, observed);
    let revision = page.revision;
    f.store
        .record_model_cost(&first, &estimate("USD", 12345), 106)
        .unwrap();
    assert_eq!(
        f.definitions.usage_ledger(query()).unwrap().revision,
        revision
    );
    assert!(
        f.store
            .record_model_cost(&first, &estimate("USD", 12346), 106)
            .is_err()
    );
    let reopen = crate::control::ControlStore::new(f.definitions.path());
    assert_eq!(
        reopen.usage_ledger(query()).unwrap().summary.costs[0]
            .amount
            .micros,
        12345
    );
    f.store
        .record_model_usage(
            &second,
            ModelTokenUsage {
                input_tokens: 20,
                output_tokens: 2,
            },
            107,
        )
        .unwrap();
    assert_eq!(
        reopen
            .usage_ledger(query())
            .unwrap()
            .summary
            .reserved_tokens,
        0
    );
    assert!(
        f.store
            .reserve_model_call(&root.lease, 3, allowance("model-0", 1, 1), 107)
            .is_err()
    );
}
#[test]
fn global_usage_pages_filter_total_scope_without_combining_currencies_or_duplicates() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    for step in 1..=40 {
        let call = f
            .store
            .reserve_model_call(
                &root.lease,
                step,
                allowance("model-0", 100, 10),
                100 + u64::from(step),
            )
            .unwrap();
        f.store
            .record_model_usage(
                &call,
                ModelTokenUsage {
                    input_tokens: step as u64,
                    output_tokens: 1,
                },
                200 + u64::from(step),
            )
            .unwrap();
        if step == 1 {
            f.store
                .record_model_cost(&call, &estimate("USD", 100), 201)
                .unwrap();
        }
        if step == 2 {
            f.store
                .record_model_cost(&call, &estimate("EUR", 200), 202)
                .unwrap();
        }
    }
    let page = f.definitions.usage_ledger(query()).unwrap();
    assert_eq!(page.entries.len(), 32);
    assert_eq!(page.summary.records, 40);
    assert_eq!(page.summary.input_tokens, 820);
    assert_eq!(page.summary.costs.len(), 2);
    let mut q = query();
    q.after = page.next_cursor;
    let older = f.definitions.usage_ledger(q).unwrap();
    assert_eq!(older.entries.len(), 8);
    assert_eq!(older.summary.records, 40);
    assert!(older.next_cursor.is_none());
    assert!(
        older
            .entries
            .iter()
            .all(|e| !page.entries.iter().any(|n| n.sequence == e.sequence))
    );
    let mut q = query();
    q.agent_id = Some(f.agents[1]);
    assert_eq!(f.definitions.usage_ledger(q).unwrap().summary.records, 0);
    let mut q = query();
    q.agent_id = Some(f.agents[0]);
    q.project_id = Some(f.project);
    q.provider = Some(ProviderKind::Nebius);
    q.environment = Some(ExecutionEnvironment::Local);
    q.model = Some("model-0".into());
    q.kind = Some(UsageLedgerKind::ModelCall);
    q.task_id = Some(root.assignment.task_id);
    q.since = Some(130);
    // Periods use call start, not a later receipt date.
    assert_eq!(f.definitions.usage_ledger(q).unwrap().summary.records, 11);
    let mut q = query();
    q.limit = 65;
    assert!(f.definitions.usage_ledger(q).is_err());
    let mut q = query();
    q.model = Some(" ".into());
    assert!(f.definitions.usage_ledger(q).is_err());
    let mut q = query();
    q.after = Some(u64::MAX);
    assert!(f.definitions.usage_ledger(q).is_err());
    let wire = serde_json::to_string(&page).unwrap();
    assert!(!wire.contains("Produce reviewable work") && !wire.contains("Implement and validate"));
}
#[test]
fn global_usage_racing_identical_costs_settle_once_and_changed_receipts_reject() {
    let f = Fixture::new();
    let run = f.run();
    let root = f.claim(run.id, run.work[0].id);
    let call = f
        .store
        .reserve_model_call(&root.lease, 1, allowance("model-0", 100, 10), 102)
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let before = f.definitions.usage_ledger(query()).unwrap().revision;
    let handles = (0..2)
        .map(|_| {
            let barrier = barrier.clone();
            let path = f.definitions.path();
            let call = call.clone();
            std::thread::spawn(move || {
                barrier.wait();
                TeamCoordinator::new(path).record_model_cost(&call, &estimate("USD", 0), 104)
            })
        })
        .collect::<Vec<_>>();
    for h in handles {
        h.join().unwrap().unwrap();
    }
    let page = f.definitions.usage_ledger(query()).unwrap();
    assert_eq!(page.revision, before + 1);
    assert_eq!(page.summary.unknown_cost_records, 0);
    assert_eq!(page.summary.costs[0].amount.micros, 0);
    assert!(
        f.store
            .record_model_cost(&call, &estimate("usd", 1), 105)
            .is_err()
    );
    assert!(
        f.store
            .record_model_cost(&call, &estimate("USD", 1_000_000_000_001), 105)
            .is_err()
    );
    assert_eq!(
        f.definitions.usage_ledger(query()).unwrap().revision,
        page.revision
    );
}
#[test]
fn global_usage_solo_calls_reconcile_exactly_without_team_or_invoice_claims() {
    let f = Fixture::new();
    let id = f
        .definitions
        .begin_solo_usage(
            f.project,
            Uuid::new_v4(),
            ProviderKind::Nebius,
            "solo-model",
            100,
        )
        .unwrap();
    let page = f.definitions.usage_ledger(query()).unwrap();
    assert_eq!(page.summary.unknown_calls, 1);
    assert_eq!(page.summary.reserved_tokens, 0);
    assert_eq!(page.summary.unknown_cost_records, 1);
    let usage = ModelTokenUsage {
        input_tokens: 40,
        output_tokens: 10,
    };
    f.definitions.settle_solo_usage(id, usage, 101).unwrap();
    let page = f.definitions.usage_ledger(query()).unwrap();
    let revision = page.revision;
    assert_eq!(page.entries[0].kind, UsageLedgerKind::SoloModelCall);
    assert!(
        page.entries[0].agent_id.is_none()
            && page.entries[0].run_id.is_none()
            && page.entries[0].cost.is_none()
    );
    f.definitions.settle_solo_usage(id, usage, 102).unwrap();
    assert_eq!(
        f.definitions.usage_ledger(query()).unwrap().revision,
        revision
    );
    assert!(
        f.definitions
            .settle_solo_usage(
                id,
                ModelTokenUsage {
                    input_tokens: 41,
                    output_tokens: 10
                },
                102
            )
            .is_err()
    );
}
