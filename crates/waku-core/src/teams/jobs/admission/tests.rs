use super::*;
use rusqlite::TransactionBehavior;
use serde_json::json;
fn policy(jobs: u64) -> Value {
    let cap = json!({"jobs":jobs,"cpu_millis":jobs*500,"memory_bytes":jobs*128*MIB,"processes":jobs*32,"storage_bytes":1024*MIB});
    json!({"max_layer_bytes":64*MIB,"resources":{"group":{"version":1,"cpu_millis":500,"memory_bytes":128*MIB,"processes":32}},
        "admission":{"version":1,"owner_limit":cap,"project_limit":cap}})
}
fn database(path: Option<&std::path::Path>) -> Connection {
    let db = path.map_or_else(
        || Connection::open_in_memory().unwrap(),
        |p| Connection::open(p).unwrap(),
    );
    db.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
    super::super::super::storage::schema(&db).unwrap();
    db
}
fn record(db: &Connection, project: Uuid, policy: &Value, state: &str, archive: usize) -> Uuid {
    let request = Uuid::new_v4();
    let run = Uuid::new_v4();
    let work = Uuid::new_v4();
    db.execute("INSERT INTO ion_team_runs(id,root_task_id,project_id,status,revision,request,body) VALUES(?,?,?,'running',1,'{}','{}')",params![run.to_string(),Uuid::new_v4().to_string(),project.to_string()]).unwrap();
    db.execute("INSERT INTO ion_team_work(id,run_id,task_id,agent_id,status,body) VALUES(?,?,?,?,'running','{}')",params![work.to_string(),run.to_string(),Uuid::new_v4().to_string(),Uuid::new_v4().to_string()]).unwrap();
    let job = super::super::JobRequest {
        version: 1,
        request_id: request,
        argv: vec!["/usr/bin/true".into()],
        timeout_seconds: 1,
        inputs: vec![],
        project_snapshot: None,
        output_paths: vec![],
    };
    db.execute("INSERT INTO ion_team_jobs(request_id,run_id,work_id,attempt_id,image,project,signature,request,state) VALUES(?,?,?,?,?,?,?,?,?)",
        params![request.to_string(),run.to_string(),work.to_string(),Uuid::new_v4().to_string(),Uuid::new_v4().to_string(),"cloud-a",json!({"policy":policy.to_string()}).to_string(),serde_json::to_string(&job).unwrap(),state]).unwrap();
    if archive > 0 {
        db.execute(
            "INSERT INTO ion_team_job_projects(request_id,archive) VALUES(?,?)",
            params![request.to_string(), vec![0u8; archive]],
        )
        .unwrap();
    }
    request
}
fn reserve_record(
    db: &mut Connection,
    project: Uuid,
    policy: &Value,
    archive: usize,
) -> Result<Uuid> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let request = record(
        &tx,
        project,
        policy,
        if archive > 0 { "staging" } else { "submitting" },
        archive,
    );
    reserve(&tx, request, project, policy, archive as u64)?;
    tx.commit()?;
    Ok(request)
}
fn set_release(db: &mut Connection, request: Uuid, compute: bool, layer: bool) {
    let tx = db.transaction().unwrap();
    if compute {
        release_compute(&tx, request).unwrap();
    }
    if layer {
        reject(&tx, request).unwrap();
    }
    tx.commit().unwrap();
}
fn summary(db: &Connection, project: Option<Uuid>) -> CommandResourceUsage {
    usage(&holds(db, None).unwrap(), project).unwrap()
}

#[test]
fn admission_owner_limit_spans_runs_projects_and_targets() {
    let mut db = database(None);
    let first = Uuid::new_v4();
    let second = Uuid::new_v4();
    reserve_record(&mut db, first, &policy(1), 0).unwrap();
    let mut next = policy(4);
    next["target"] = json!(Uuid::new_v4());
    assert!(reserve_record(&mut db, second, &next, 0).is_err());
    assert_eq!(summary(&db, None).used.jobs, 1);
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_jobs", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn admission_project_limit_is_independent_but_owner_combines_projects() {
    let mut db = database(None);
    let one = Uuid::new_v4();
    let two = Uuid::new_v4();
    let mut p = policy(3);
    p["admission"]["project_limit"] = policy(1)["admission"]["project_limit"].clone();
    reserve_record(&mut db, one, &p, 0).unwrap();
    reserve_record(&mut db, two, &p, 0).unwrap();
    assert!(reserve_record(&mut db, one, &p, 0).is_err());
    assert_eq!(summary(&db, None).used.jobs, 2);
    assert_eq!(summary(&db, Some(one)).used.jobs, 1);
    assert_eq!(summary(&db, Some(Uuid::new_v4())).used.jobs, 0);
}
#[test]
fn admission_tightening_intersects_each_resource_dimension() {
    for field in ["cpu_millis", "memory_bytes", "processes", "storage_bytes"] {
        let mut db = database(None);
        let project = Uuid::new_v4();
        let p = policy(3);
        reserve_record(&mut db, project, &p, 0).unwrap();
        let mut next = p.clone();
        let amount = summary(&db, None).used;
        let cap = match field {
            "cpu_millis" => amount.cpu_millis,
            "memory_bytes" => amount.memory_bytes,
            "processes" => amount.processes,
            _ => amount.storage_bytes,
        };
        next["admission"]["owner_limit"][field] = json!(cap);
        next["admission"]["project_limit"][field] = json!(cap);
        assert!(
            reserve_record(&mut db, project, &next, 0).is_err(),
            "{field}"
        );
    }
}
#[test]
fn admission_registry_expansion_does_not_enlarge_outstanding_caps() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    reserve_record(&mut db, project, &policy(1), 0).unwrap();
    assert!(reserve_record(&mut db, project, &policy(128), 0).is_err());
    assert_eq!(summary(&db, None).limit.unwrap().jobs, 1);
}
#[test]
fn admission_omission_cannot_bypass_old_cap_on_new_target() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    reserve_record(&mut db, project, &policy(2), 0).unwrap();
    let mut next = policy(2);
    next.as_object_mut().unwrap().remove("admission");
    next["target"] = json!(Uuid::new_v4());
    assert!(reserve_record(&mut db, project, &next, 0).is_err());
}
#[test]
fn admission_unknown_historical_active_group_is_not_zero() {
    for state in ["staging", "submitting", "running", "canceling", "failed"] {
        let mut db = database(None);
        let project = Uuid::new_v4();
        let mut old = policy(2);
        old.as_object_mut().unwrap().remove("admission");
        old["resources"] = Value::Null;
        record(&db, project, &old, state, 0);
        assert_eq!(summary(&db, None).unknown_jobs, 1);
        assert!(
            reserve_record(&mut db, project, &policy(2), 0).is_err(),
            "{state}"
        );
    }
}
#[test]
fn admission_known_legacy_groups_count_without_legacy_caps() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let mut old = policy(2);
    old.as_object_mut().unwrap().remove("admission");
    record(&db, project, &old, "running", 0);
    reserve_record(&mut db, project, &policy(2), 0).unwrap();
    assert_eq!(summary(&db, None).used.jobs, 2);
    assert!(reserve_record(&mut db, project, &policy(3), 0).is_err());
}
#[test]
fn admission_absence_preserves_old_unknown_group_submission() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let mut old = policy(2);
    old.as_object_mut().unwrap().remove("admission");
    old["resources"] = Value::Null;
    reserve_record(&mut db, project, &old, 0).unwrap();
    reserve_record(&mut db, project, &old, 0).unwrap();
    assert_eq!(summary(&db, None).unknown_jobs, 2);
    assert!(summary(&db, None).limit.is_none());
}
#[test]
fn admission_requires_real_group_and_finite_native_layer_witness() {
    for change in [
        json!({"resources":null}),
        json!({"max_layer_bytes":null}),
        json!({"max_layer_bytes":0}),
        json!({"max_layer_bytes":u64::MAX}),
    ] {
        let mut p = policy(2);
        for (key, value) in change.as_object().unwrap() {
            p[key] = value.clone();
        }
        assert!(validate_policy(&p).is_err());
    }
    let mut p = policy(2);
    p["admission"]["owner_limit"]["extra"] = json!(true);
    assert!(validate_policy(&p).is_err());
    let mut p = policy(2);
    p["admission"]["version"] = json!(2);
    assert!(validate_policy(&p).is_err());
}
#[test]
fn admission_vectors_have_checked_arithmetic() {
    for field in [0, 1, 2, 3, 4] {
        let mut current = CommandResourceVector::default();
        let mut next = CommandResourceVector::default();
        match field {
            0 => {
                current.jobs = u64::MAX;
                next.jobs = 1
            }
            1 => {
                current.cpu_millis = u64::MAX;
                next.cpu_millis = 1
            }
            2 => {
                current.memory_bytes = u64::MAX;
                next.memory_bytes = 1
            }
            3 => {
                current.processes = u64::MAX;
                next.processes = 1
            }
            _ => {
                current.storage_bytes = u64::MAX;
                next.storage_bytes = 1
            }
        }
        assert!(current.add(&next).is_err());
    }
    let bad = Allocation {
        group: None,
        layer_bytes: Some(1),
        archive_bytes: u64::MAX,
    };
    assert!(bad.held(true, false).is_err());
}
#[test]
fn admission_terminal_or_stop_status_does_not_release_compute() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let request = reserve_record(&mut db, project, &policy(1), 0).unwrap();
    for state in ["canceling", "failed", "settled"] {
        db.execute(
            "UPDATE ion_team_jobs SET state=? WHERE request_id=?",
            params![state, request.to_string()],
        )
        .unwrap();
        assert_eq!(summary(&db, None).used.jobs, 1);
        assert!(reserve_record(&mut db, project, &policy(2), 0).is_err());
    }
}
#[test]
fn admission_verified_receipt_releases_compute_but_retains_storage() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let first = reserve_record(&mut db, project, &policy(1), 1024).unwrap();
    set_release(&mut db, first, true, false);
    let used = summary(&db, None);
    assert_eq!(used.used.jobs, 0);
    assert_eq!(used.used.cpu_millis, 0);
    assert_eq!(used.used.storage_bytes, 64 * MIB + 2048);
    assert_eq!(used.retained_archive_bytes, 1024);
    reserve_record(&mut db, project, &policy(2), 0).unwrap();
    assert_eq!(summary(&db, None).used.jobs, 1);
}
#[test]
fn admission_definitive_rejection_releases_layer_but_keeps_archive_and_cap() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let first = reserve_record(&mut db, project, &policy(1), 1024).unwrap();
    set_release(&mut db, first, false, true);
    let used = summary(&db, None);
    assert_eq!(used.used.jobs, 0);
    assert_eq!(used.used.storage_bytes, 2048);
    assert_eq!(used.limit.unwrap().jobs, 1);
    let mut omit = policy(2);
    omit.as_object_mut().unwrap().remove("admission");
    assert!(reserve_record(&mut db, project, &omit, 0).is_err());
}
#[test]
fn admission_rejection_without_retention_releases_all_capacity_and_cap() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let first = reserve_record(&mut db, project, &policy(1), 0).unwrap();
    set_release(&mut db, first, false, true);
    let used = summary(&db, None);
    assert_eq!(used.used, CommandResourceVector::default());
    assert!(used.limit.is_none());
    let mut omit = policy(2);
    omit.as_object_mut().unwrap().remove("admission");
    reserve_record(&mut db, project, &omit, 0).unwrap();
}
#[test]
fn admission_archive_budget_is_atomic_with_job_and_blob() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let mut p = policy(2);
    p["admission"]["owner_limit"]["storage_bytes"] = json!(64 * MIB);
    p["admission"]["project_limit"]["storage_bytes"] = json!(64 * MIB);
    assert!(reserve_record(&mut db, project, &p, 1024).is_err());
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_jobs", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_job_projects", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM ion_team_job_admission", [], |r| r
            .get::<_, u64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn admission_reopen_preserves_lost_ack_and_retained_storage() {
    let folder = std::env::temp_dir().join(format!("ion-admission-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let file = folder.join("control.db");
    let project = Uuid::new_v4();
    let mut db = database(Some(&file));
    let first = reserve_record(&mut db, project, &policy(1), 256).unwrap();
    drop(db);
    let db = database(Some(&file));
    assert_eq!(summary(&db, None).used.jobs, 1);
    assert_eq!(summary(&db, None).retained_archive_bytes, 256);
    assert_eq!(
        db.query_row(
            "SELECT state FROM ion_team_jobs WHERE request_id=?",
            [first.to_string()],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "staging"
    );
    drop(db);
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(folder).unwrap();
}
#[test]
fn admission_parallel_reservation_cannot_oversubscribe() {
    let folder = std::env::temp_dir().join(format!("ion-admission-{}", Uuid::new_v4()));
    std::fs::create_dir(&folder).unwrap();
    let file = folder.join("control.db");
    drop(database(Some(&file)));
    let project = Uuid::new_v4();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let path = file.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut db = database(Some(&path));
                barrier.wait();
                reserve_record(&mut db, project, &policy(1), 0).is_ok()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        handles
            .into_iter()
            .map(|h| usize::from(h.join().unwrap()))
            .sum::<usize>(),
        1
    );
    let db = database(Some(&file));
    assert_eq!(summary(&db, None).used.jobs, 1);
    drop(db);
    std::fs::remove_file(file).unwrap();
    std::fs::remove_dir(folder).unwrap();
}

#[test]
fn admission_durable_allocation_cannot_diverge_from_original_policy() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let request = reserve_record(&mut db, project, &policy(2), 0).unwrap();
    db.execute("UPDATE ion_team_job_admission SET allocation=json_set(allocation,'$.group.cpu_millis',100) WHERE request_id=?",[request.to_string()]).unwrap();
    assert!(holds(&db, None).is_err());
    assert!(reserve_record(&mut db, project, &policy(2), 0).is_err());
}
#[test]
fn admission_legacy_terminal_receipt_requires_full_request_and_operation() {
    for valid in [false, true] {
        let db = database(None);
        let project = Uuid::new_v4();
        let mut old = policy(2);
        old.as_object_mut().unwrap().remove("admission");
        let request = record(&db, project, &old, "settled", 0);
        let mut receipt = json!({"version":1,"request_id":request,"exit_code":0,"timed_out":false,"canceled":false,"elapsed_milliseconds":0,
            "stdout":{"content_base64":"","bytes":0,"truncated":false},"stderr":{"content_base64":"","bytes":0,"truncated":false},"outputs":[],"output_errors":[]});
        if !valid {
            receipt["request_id"] = json!(Uuid::new_v4());
        }
        db.execute(
            "UPDATE ion_team_jobs SET receipt=?,operation=? WHERE request_id=?",
            params![
                receipt.to_string(),
                Uuid::new_v4().to_string(),
                request.to_string()
            ],
        )
        .unwrap();
        assert_eq!(summary(&db, None).used.jobs, u64::from(!valid));
        db.execute(
            "UPDATE ion_team_jobs SET operation=NULL WHERE request_id=?",
            [request.to_string()],
        )
        .unwrap();
        assert_eq!(summary(&db, None).used.jobs, 1);
    }
}

#[test]
fn admission_unknown_storage_is_not_hidden_when_known_compute_is_zero() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let mut old = policy(2);
    old.as_object_mut().unwrap().remove("admission");
    old["max_layer_bytes"] = Value::Null;
    let request = reserve_record(&mut db, project, &old, 0).unwrap();
    set_release(&mut db, request, true, false);
    let used = summary(&db, None);
    assert_eq!(used.used, CommandResourceVector::default());
    assert_eq!(used.unknown_jobs, 1);
    assert!(reserve_record(&mut db, project, &policy(2), 0).is_err());
}

#[test]
fn admission_unconfigured_historical_path_does_not_gain_global_history_cap() {
    let mut db = database(None);
    let project = Uuid::new_v4();
    let mut old = policy(2);
    old.as_object_mut().unwrap().remove("admission");
    old["resources"] = Value::Null;
    let original = record(&db, project, &old, "failed", 0);
    db.execute("WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x+1 FROM n WHERE x<2048)
       INSERT INTO ion_team_jobs(request_id,run_id,work_id,attempt_id,image,project,signature,request,state)
       SELECT 'legacy-'||n.x,j.run_id,j.work_id,j.attempt_id,j.image,j.project,j.signature,j.request,'failed'
       FROM n,ion_team_jobs j WHERE j.request_id=?",[original.to_string()]).unwrap();
    assert!(holds(&db, None).is_err());
    reserve_record(&mut db, project, &old, 0).unwrap();
    assert!(reserve_record(&mut db, project, &policy(2), 0).is_err());
}
