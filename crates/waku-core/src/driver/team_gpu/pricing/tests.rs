use super::*;
use crate::driver::team_gpu::{
    preflight,
    tests::{Fixture, bytes, client, launch},
};
use crate::driver::team_test_http::Server;
use crate::teams::gpu_jobs::{GpuEstimateMethod, GpuPriceComponent, GpuPriceSku};
pub(in crate::driver::team_gpu) fn policy() -> GpuPricePolicy {
    use GpuPriceComponent::*;
    GpuPricePolicy {
        version: 1,
        method: GpuEstimateMethod::NativeJobV1,
        max_hourly_micros: 5000,
        startup_seconds: 300,
        cleanup_seconds: 300,
        extra_reservation_micros: 100,
        skus: [
            ("sku-gpu", "1", Gpu),
            ("sku-vcpu", "8", Vcpu),
            ("sku-ram", "32", MemoryGib),
            ("sku-disk", "16", DiskBytes),
        ]
        .into_iter()
        .map(|(id, quantity, component)| GpuPriceSku {
            id: id.into(),
            quantity: quantity.into(),
            covers: vec![component],
        })
        .collect(),
    }
}
pub(in crate::driver::team_gpu) fn value(l: &Launch) -> Value {
    let rows = json!([
        {"skuId":"sku-gpu","quantity":"1","cost":"0.002","costRounded":"777"},
        {"skuId":"sku-vcpu","quantity":"8","cost":"0.001","costRounded":"777"},
        {"skuId":"sku-ram","quantity":"32","cost":"0.0009","costRounded":"777"},
        {"skuId":"sku-disk","quantity":"16","cost":"0.0001","costRounded":"777"}]);
    let total = json!({"skuCosts":rows,"cost":"0.004","costRounded":"777"});
    let unit = json!({"unit":"hour","unitType":"UNIT_TYPE_TIME_BASED"});
    json!({"currency":"usd","resourceCosts":[{"metadata":{"parentId":l.project,"name":l.name()},"aggregationUnit":unit,"general":{"total":total}}],
        "totalCosts":[{"aggregationUnit":unit,"general":{"total":total}}],"unknown":"fixture-private-not-persisted"})
}
fn setup() -> (Fixture, Launch) {
    let mut f = Fixture::new(true, true);
    let l = launch();
    f.policy.resources = Some(preflight::tests::policy(&l));
    (f, l)
}
#[test]
fn team_gpu_price_native_any_exact_skus_ceiling_and_receipt_commit_before_one_create() {
    let (f, l) = setup();
    let body = l.body().unwrap();
    let path = f.dir.join("control.db");
    let id = l.request.request_id;
    let mut replies = preflight::tests::launch_responses(&l, f.policy.resources.as_ref().unwrap());
    replies.push((
        200,
        bytes(json!({"id":"operation-create","resourceId":"aijob-fixture"})),
    ));
    let server = Server::with_hook(replies, move |r| {
        if r.line.starts_with("POST /ai/v1/jobs ") {
            let db = rusqlite::Connection::open(&path).unwrap();
            let saved: String = db
                .query_row(
                    "SELECT body FROM ion_team_gpu_prices WHERE request_id=?",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            let p: Value = serde_json::from_str(&saved).unwrap();
            assert_eq!(p["request_id"], id.to_string());
            assert_eq!(p["currency"], "USD");
            assert_eq!(p["hourly_cost"], "0.004");
            assert_eq!(p["horizon_seconds"], 4200);
            assert_eq!(p["estimated_micros"], 4667);
            assert_eq!(p["required_micros"], 4767);
            assert_eq!(p["skus"].as_array().unwrap().len(), 4);
            assert!(
                !saved.contains("fixture-only")
                    && !saved.contains("fixture-private")
                    && !saved.contains("costRounded")
                    && !saved.contains("quantityRounded")
                    && !saved.contains("gpu fixture")
            );
            let count: u64 = db
                .query_row(
                    "SELECT COUNT(*) FROM ion_team_gpu_preflight WHERE request_id=?",
                    [id.to_string()],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1);
        }
    });
    let c = client(&server);
    let saved = launch_once_with(&c, &f.coordinator, &f.lease, &l, &f.policy).unwrap();
    let restart = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        launch_once_with(&c, &restart, &f.lease, &l, &f.policy)
            .unwrap()
            .job_id,
        saved.job_id
    );
    let requests = server.finish();
    assert_eq!(requests.len(), 15);
    let estimate = &requests[13];
    assert!(
        estimate
            .line
            .starts_with("POST /billing/v1/calculator/estimate-batch ")
    );
    let mut any = body.clone();
    any["@type"] = json!("type.googleapis.com/nebius.ai.v1.CreateJobRequest");
    assert_eq!(estimate.body["resourceSpecs"], json!([{"spec":any}]));
    assert_eq!(estimate.body["currency"], "usd");
    assert_eq!(
        estimate.body["offerTypes"],
        json!(["OFFER_TYPE_CONTRACT_PRICE"])
    );
    assert_eq!(
        estimate.body["filterAggregationUnit"],
        json!({"filterAggregationUnitValues":["FILTER_AGGREGATION_UNIT_HOUR"]})
    );
    assert!(
        estimate
            .headers
            .to_ascii_lowercase()
            .contains("authorization: bearer fixture-only")
    );
    assert_eq!(requests[14].body, body);
    let reserved: u64 = rusqlite::Connection::open(f.dir.join("control.db"))
        .unwrap()
        .query_row(
            "SELECT reserved_micros FROM ion_team_gpu_jobs WHERE request_id=?",
            [id.to_string()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reserved, 10000); // Estimate/terminal state never releases the approved allowance.
}
#[test]
fn team_gpu_price_missing_or_invalid_owner_approval_fails_before_reads_or_reservation() {
    let (f, l) = setup();
    let server = Server::new(vec![]);
    let c = client(&server);
    for mode in 0..11 {
        let mut b = f.policy.clone();
        let p = b.pricing.as_mut().unwrap();
        match mode {
            0 => b.pricing = None,
            1 => p.version = 2,
            2 => p.max_hourly_micros = 0,
            3 => p.startup_seconds = 86401,
            4 => p.skus.pop().map(|_| ()).unwrap(),
            5 => p.skus[0].quantity = "-1".into(),
            6 => p.skus[0].quantity = "0".into(),
            7 => p.skus[1].id = p.skus[0].id.clone(),
            8 => p.skus[1].covers = p.skus[0].covers.clone(),
            9 => p.skus[0].id = "../unapproved".into(),
            _ => b.currency = "EUR".into(),
        };
        assert!(
            launch_once_with(&c, &f.coordinator, &f.lease, &l, &b).is_err(),
            "mode {mode}"
        );
    }
    assert!(server.finish().is_empty());
    assert!(
        f.coordinator
            .gpu_jobs(f.lease.attempt_id())
            .unwrap()
            .is_empty()
    );
}
#[test]
fn team_gpu_price_changed_incomplete_ambiguous_or_over_budget_native_estimates_never_create() {
    let cases = vec![
        ("/currency", json!("ils")),
        ("/resourceCosts", json!([])),
        ("/totalCosts", json!([])),
        ("/resourceCosts/0/metadata/parentId", json!("project-other")),
        ("/resourceCosts/0/metadata/name", json!("job-other")),
        (
            "/resourceCosts/0/overrideMetadata",
            json!({"parentId":"other"}),
        ),
        ("/resourceCosts/0/aggregationUnit/unit", json!("month")),
        (
            "/resourceCosts/0/aggregationUnit/unitType",
            json!("UNIT_TYPE_USAGE_BASED"),
        ),
        ("/totalCosts/0/aggregationUnit/unit", json!("request")),
        ("/totalCosts/0/aggregationUnit/unitType", json!(null)),
        ("/resourceCosts/0/general/total/cost", json!("0.0040001")),
        ("/resourceCosts/0/general/total/cost", json!(0.004)),
        (
            "/resourceCosts/0/general/total/skuCosts/0/skuId",
            json!("sku-other"),
        ),
        (
            "/resourceCosts/0/general/total/skuCosts/0/quantity",
            json!("2"),
        ),
        (
            "/resourceCosts/0/general/total/skuCosts/0/quantity",
            json!(1),
        ),
        (
            "/resourceCosts/0/general/total/skuCosts/0/cost",
            json!("-1"),
        ),
        (
            "/resourceCosts/0/general/total/skuCosts/0/cost",
            json!("1e-19"),
        ),
        ("/totalCosts/0/general/total/cost", json!("0.005")),
        ("/resourceCosts/0/general", json!(null)),
        ("/totalCosts/0/general", json!(null)),
    ];
    let (f, l) = setup();
    for (path, new) in cases {
        let mut v = value(&l);
        if path == "/resourceCosts/0/overrideMetadata" {
            v["resourceCosts"][0]["overrideMetadata"] = new;
        } else {
            *v.pointer_mut(path).unwrap() = new;
        }
        let mut replies = preflight::tests::responses(&l, f.policy.resources.as_ref().unwrap());
        replies.push((200, bytes(v)));
        let server = Server::new(replies);
        assert!(
            launch_once_with(&client(&server), &f.coordinator, &f.lease, &l, &f.policy).is_err(),
            "{path}"
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 14);
        assert!(requests.iter().all(|r| !r.line.starts_with("POST /ai/")));
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
    }
    for mode in 0..8 {
        let mut v = value(&l);
        let mut b = f.policy.clone();
        match mode {
            0 => {
                let row = v["resourceCosts"][0].clone();
                v["resourceCosts"].as_array_mut().unwrap().push(row);
            }
            1 => {
                let row = v["totalCosts"][0].clone();
                v["totalCosts"].as_array_mut().unwrap().push(row);
            }
            2 => v["resourceCosts"][0]["autoscale"] = json!({}),
            3 => v["totalCosts"][0]["range"] = json!({}),
            4 => {
                v["resourceCosts"][0]["general"]["total"]["skuCosts"][1]["skuId"] = json!("sku-gpu")
            }
            5 => b.pricing.as_mut().unwrap().max_hourly_micros = 3999,
            6 => b.reservation_micros = 4766,
            _ => b.pricing.as_mut().unwrap().extra_reservation_micros = 1_000_000,
        };
        let mut replies = preflight::tests::responses(&l, b.resources.as_ref().unwrap());
        replies.push((200, bytes(v)));
        let server = Server::new(replies);
        assert!(
            launch_once_with(&client(&server), &f.coordinator, &f.lease, &l, &b).is_err(),
            "mode {mode}"
        );
        assert_eq!(server.finish().len(), 14);
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
    }
}
#[test]
fn team_gpu_price_unsupported_job_estimation_denied_status_or_redirect_does_not_substitute_compute_or_launch()
 {
    let (f, l) = setup();
    for status in [400, 403, 404, 500, 302] {
        let mut replies = preflight::tests::responses(&l, f.policy.resources.as_ref().unwrap());
        replies.push((
            status,
            bytes(json!({"code":12,"message":"fixture-private-unsupported"})),
        ));
        let server = Server::new(replies);
        assert!(
            launch_once_with(&client(&server), &f.coordinator, &f.lease, &l, &f.policy).is_err()
        );
        let requests = server.finish();
        assert_eq!(requests.len(), 14);
        assert!(requests[13].line.starts_with("POST /billing/"));
        assert_eq!(
            requests[13].body["resourceSpecs"][0]["spec"]["@type"],
            "type.googleapis.com/nebius.ai.v1.CreateJobRequest"
        );
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
        assert!(!f.coordinator.has_pending_effect(f.lease.run_id()).unwrap());
    }
}
#[test]
fn team_gpu_price_receipt_freshness_original_policy_body_and_request_are_checked_atomically() {
    let (f, l) = setup();
    let server = Server::new(preflight::tests::launch_responses(
        &l,
        f.policy.resources.as_ref().unwrap(),
    ));
    let c = client(&server);
    let resources = c.verify_preflight(&l, &f.policy).unwrap();
    let price = c.verify_price(&l, &f.policy).unwrap();
    server.finish();
    let now = crate::model::unix_time();
    let body = l.body().unwrap();
    for mode in 0..6 {
        let mut b = f.policy.clone();
        let mut r = price.clone();
        let mut request = l.request.clone();
        let mut body = body.clone();
        match mode {
            0 => r.checked_at = now - 31,
            1 => r.checked_at = now + 1,
            2 => b.pricing.as_mut().unwrap().max_hourly_micros += 1,
            3 => body["spec"]["image"] = json!(format!("other/image@sha256:{}", "b".repeat(64))),
            4 => request.request_id = uuid::Uuid::new_v4(),
            _ => b.reservation_micros += 1,
        };
        assert!(
            f.coordinator
                .reserve_verified_gpu_launch(&f.lease, &request, &body, &b, &resources, &r, now)
                .is_err(),
            "mode {mode}"
        );
        assert!(
            f.coordinator
                .gpu_jobs(f.lease.attempt_id())
                .unwrap()
                .is_empty()
        );
    }
    assert!(
        f.coordinator
            .reserve_verified_gpu_launch(
                &f.lease, &l.request, &body, &f.policy, &resources, &price, now
            )
            .unwrap()
    );
    let restart = TeamCoordinator::new(f.dir.join("control.db"));
    assert!(
        !restart
            .reserve_verified_gpu_launch(
                &f.lease,
                &l.request,
                &body,
                &f.policy,
                &resources,
                &price,
                now + 31
            )
            .unwrap()
    );
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let count: u64 = db
        .query_row("SELECT COUNT(*) FROM ion_team_gpu_prices", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}
#[test]
fn team_gpu_price_legacy_missing_policy_preserves_exact_replay_but_rejects_new_launch_without_reads()
 {
    let (mut f, l) = setup();
    f.policy.pricing = None;
    let encoded = serde_json::to_value(&f.policy).unwrap();
    assert!(encoded.get("pricing").is_none());
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<GpuBudgetPolicy>(encoded.clone()).unwrap())
            .unwrap(),
        encoded
    );
    let server = Server::new(preflight::tests::responses(
        &l,
        f.policy.resources.as_ref().unwrap(),
    ));
    let resource = client(&server).verify_preflight(&l, &f.policy).unwrap();
    server.finish();
    f.coordinator
        .reserve_resource_gpu_launch_for_test(
            &f.lease,
            &l.request,
            &l.body().unwrap(),
            &f.policy,
            &resource,
            crate::model::unix_time(),
        )
        .unwrap();
    let server = Server::new(vec![]);
    let c = client(&server);
    let restart = TeamCoordinator::new(f.dir.join("control.db"));
    assert_eq!(
        launch_once_with(&c, &restart, &f.lease, &l, &f.policy)
            .unwrap()
            .state,
        "submitting"
    );
    assert!(
        launch_once_with(&c, &restart, &f.lease, &launch(), &f.policy)
            .err()
            .unwrap()
            .to_string()
            .contains("native job estimator")
    );
    assert!(server.finish().is_empty());
    assert!(restart.has_pending_effect(f.lease.run_id()).unwrap());
    let db = rusqlite::Connection::open(f.dir.join("control.db")).unwrap();
    let count: u64 = db
        .query_row("SELECT COUNT(*) FROM ion_team_gpu_prices", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 0);
}
#[test]
fn team_gpu_price_exact_formats_reordering_bundled_components_and_lifecycle_allowance() {
    let (f, l) = setup();
    let mut v = value(&l);
    for key in ["resourceCosts", "totalCosts"] {
        let total = &mut v[key][0]["general"]["total"];
        total["cost"] = json!("4e-3");
        let rows = total["skuCosts"].as_array_mut().unwrap();
        rows.reverse();
        let gpu = rows.iter_mut().find(|s| s["skuId"] == "sku-gpu").unwrap();
        gpu["quantity"] = json!("1.000");
        gpu["cost"] = json!("2e-3");
    }
    let server = Server::new(vec![(200, bytes(v))]);
    let price = client(&server).verify_price(&l, &f.policy).unwrap();
    assert_eq!(price.required_micros, 4767);
    assert_eq!(server.finish().len(), 1);
    let mut b = f.policy.clone();
    let p = b.pricing.as_mut().unwrap();
    p.skus = vec![
        GpuPriceSku {
            id: "sku-preset".into(),
            quantity: "1".into(),
            covers: vec![
                GpuPriceComponent::Gpu,
                GpuPriceComponent::Vcpu,
                GpuPriceComponent::MemoryGib,
            ],
        },
        GpuPriceSku {
            id: "sku-disk".into(),
            quantity: "16".into(),
            covers: vec![GpuPriceComponent::DiskBytes],
        },
    ];
    let mut v = value(&l);
    for key in ["resourceCosts", "totalCosts"] {
        v[key][0]["general"]["total"]["skuCosts"] = json!([
        {"skuId":"sku-preset","quantity":"1","cost":"0.0039"},{"skuId":"sku-disk","quantity":"16","cost":"0.0001"}]);
    }
    let server = Server::new(vec![(200, bytes(v))]);
    assert_eq!(
        client(&server)
            .verify_price(&l, &b)
            .unwrap()
            .required_micros,
        4767
    );
    server.finish();
    b.reservation_micros = 4767;
    let mut longer = launch();
    longer.timeout_seconds = 3601;
    let server = Server::new(vec![(200, bytes(value(&longer)))]);
    b.pricing = Some(policy());
    assert!(client(&server).verify_price(&longer, &b).is_err());
    server.finish();
}
