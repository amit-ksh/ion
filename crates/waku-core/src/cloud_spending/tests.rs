use super::*;
use crate::{
    control::ControlStore,
    driver::team_gpu::tests::{Fixture, launch, launch_body},
};
fn store(f: &Fixture) -> ControlStore {
    ControlStore::new(f.dir.join("control.db"))
}
fn budget(f: &Fixture) -> CloudBudget {
    store(f).cloud_budgets(None, 64).unwrap().budgets[0]
        .budget
        .clone()
}
fn save(f: &Fixture, b: CloudBudget) -> Result<()> {
    let s = store(f);
    s.save(s.load()?.revision, ControlRecord::CloudBudget(b))?;
    Ok(())
}
fn reserve(f: &Fixture) -> Result<Uuid> {
    let l = launch();
    f.coordinator.reserve_gpu_launch(
        &f.lease,
        &l.request,
        &launch_body(&l),
        &f.policy,
        crate::model::unix_time(),
    )?;
    Ok(l.request.request_id)
}
#[test]
fn launch_replay_reserves_once_and_owner_pause_does_not_destroy_its_receipt() {
    let f = Fixture::new(true, true);
    let l = launch();
    let body = launch_body(&l);
    let now = crate::model::unix_time();
    assert!(
        f.coordinator
            .reserve_gpu_launch(&f.lease, &l.request, &body, &f.policy, now)
            .unwrap()
    );
    let mut b = budget(&f);
    b.paused = true;
    b.limit_micros = f.policy.reservation_micros;
    save(&f, b).unwrap();
    assert!(
        !f.coordinator
            .reserve_gpu_launch(&f.lease, &l.request, &body, &f.policy, now)
            .unwrap()
    );
    let page = store(&f).cloud_budgets(None, 64).unwrap();
    assert_eq!(page.budgets[0].requests, 1);
    assert_eq!(page.budgets[0].available_micros, 0);
    assert!(reserve(&f).is_err());
}
#[test]
fn separate_runs_compete_for_one_atomic_project_allowance() {
    let f = Fixture::new(true, true);
    let other = Fixture::new_in(f.dir.clone(), true, true);
    let mut b = budget(&f);
    b.limit_micros = f.policy.reservation_micros;
    save(&f, b).unwrap();
    let one = std::thread::scope(|scope| {
        let a = scope.spawn(|| reserve(&f));
        let b = scope.spawn(|| reserve(&other));
        usize::from(a.join().unwrap().is_ok()) + usize::from(b.join().unwrap().is_ok())
    });
    assert_eq!(one, 1);
    let page = store(&f).cloud_budgets(None, 64).unwrap();
    assert_eq!(page.budgets[0].requests, 1);
    assert_eq!(page.budgets[0].held_micros, f.policy.reservation_micros);
    assert_eq!(
        f.coordinator.gpu_jobs(f.lease.attempt_id()).unwrap().len()
            + other
                .coordinator
                .gpu_jobs(other.lease.attempt_id())
                .unwrap()
                .len(),
        1
    );
}
#[test]
fn exhausted_budget_rolls_back_launch_records() {
    let f = Fixture::new(true, true);
    let mut b = budget(&f);
    b.limit_micros = f.policy.reservation_micros - 1;
    save(&f, b).unwrap();
    assert!(reserve(&f).is_err());
    assert!(
        f.coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store(&f).cloud_budgets(None, 64).unwrap().budgets[0].requests,
        0
    );
}
#[test]
fn budget_edit_cannot_release_hold_change_identity_or_create_a_second_cap() {
    let f = Fixture::new(true, true);
    reserve(&f).unwrap();
    let original = budget(&f);
    for mutate in [0, 1, 2] {
        let mut b = original.clone();
        match mutate {
            0 => b.limit_micros = 1,
            1 => b.currency = "EUR".into(),
            _ => b.cloud_project = "another-project".into(),
        };
        assert!(save(&f, b).is_err());
    }
    let mut duplicate = original.clone();
    duplicate.id = Uuid::new_v4();
    assert!(save(&f, duplicate).is_err());
    let s = store(&f);
    assert!(s.remove(s.load().unwrap().revision, original.id).is_err());
    let mut raised = original;
    raised.limit_micros += 1;
    raised.paused = true;
    save(&f, raised).unwrap();
    assert_eq!(
        s.cloud_budgets(None, 64).unwrap().budgets[0].held_micros,
        f.policy.reservation_micros
    );
}
#[test]
fn currency_target_and_missing_budget_are_checked_before_launch() {
    let f = Fixture::new(true, true);
    let mut b = budget(&f);
    b.targets = vec!["different-target".into()];
    save(&f, b.clone()).unwrap();
    assert!(reserve(&f).is_err());
    b.targets = vec![f.policy.target.clone()];
    b.currency = "EUR".into();
    save(&f, b.clone()).unwrap();
    assert!(reserve(&f).is_err());
    let s = store(&f);
    s.remove(s.load().unwrap().revision, b.id).unwrap();
    assert!(reserve(&f).is_err());
}
#[test]
fn legacy_reservations_are_adopted_atomically_without_resetting_the_cap() {
    let f = Fixture::new(true, true);
    reserve(&f).unwrap();
    let s = store(&f);
    let db = s.connect().unwrap();
    db.execute("DELETE FROM ion_cloud_spending_holds", [])
        .unwrap();
    let mut b = budget(&f);
    b.limit_micros = 1;
    assert!(save(&f, b.clone()).is_err());
    assert_eq!(totals(&db, b.id).unwrap().1, 0);
    b.limit_micros = f.policy.reservation_micros;
    save(&f, b).unwrap();
    assert_eq!(
        s.cloud_budgets(None, 64).unwrap().budgets[0].available_micros,
        0
    );
}
#[test]
fn budget_pages_and_holds_survive_restart_and_reject_invalid_cursor_bounds() {
    let f = Fixture::new(true, true);
    reserve(&f).unwrap();
    let mut b = budget(&f);
    b.id = Uuid::new_v4();
    b.cloud_project = "another-project".into();
    save(&f, b).unwrap();
    let s = store(&f);
    let page = s.cloud_budgets(None, 1).unwrap();
    assert_eq!(page.total, 2);
    let next = s.cloud_budgets(page.next_cursor, 1).unwrap();
    assert_ne!(page.budgets[0].budget.id, next.budgets[0].budget.id);
    assert!(next.next_cursor.is_none());
    assert_eq!(page.budgets[0].requests + next.budgets[0].requests, 1);
    assert!(s.cloud_budgets(None, 0).is_err());
    assert!(s.cloud_budgets(None, 65).is_err());
}
