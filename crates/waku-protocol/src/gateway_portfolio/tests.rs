use super::*;
#[test]
fn portfolio_queries_represent_reads_only_and_limit_native_pages() {
    let query: PortfolioQuery =
        serde_json::from_value(json!({"kind":"projects","query":{"after":null,"limit":32}}))
            .unwrap();
    assert!(query.validate().is_ok());
    for value in [
        json!({"kind":"start","definitionId":Uuid::new_v4()}),
        json!({"kind":"projects","query":{"after":null,"limit":32,"path":"/secrets"}}),
        json!({"kind":"projects","query":{"after":null,"limit":32},"command":"shell"}),
    ] {
        assert!(serde_json::from_value::<PortfolioQuery>(value).is_err());
    }
    for limit in [0, 33] {
        let query: PortfolioQuery =
            serde_json::from_value(json!({"kind":"projects","query":{"after":null,"limit":limit}}))
                .unwrap();
        assert!(query.validate().is_err());
    }
}
#[test]
fn project_paging_reaches_old_projects_without_paths_or_unknown_totals() {
    let projects: Vec<_> = (1..=75)
        .rev()
        .map(|id| Project {
            id: Uuid::from_u128(id),
            name: "🟢".repeat(100),
            path: "/private/workspace".into(),
            created_at: 0,
        })
        .collect();
    let mut after = None;
    let mut seen = Vec::new();
    loop {
        let result = projects_result(
            &projects,
            &PortfolioProjectsQuery { after, limit: 32 },
            1000,
        )
        .unwrap();
        assert_eq!(result["page"]["total"], 75);
        assert!(!result.to_string().contains("private"));
        for row in result["page"]["projects"].as_array().unwrap() {
            seen.push(row["id"].as_str().unwrap().to_owned());
            assert_eq!(row["name"].as_str().unwrap().chars().count(), 80);
        }
        after = result["page"]["nextCursor"]
            .as_str()
            .map(|id| Uuid::parse_str(id).unwrap());
        if after.is_none() {
            break;
        }
    }
    assert_eq!(seen.len(), 75);
    assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
}
#[test]
fn unicode_inbox_transfer_bounds_whole_rows_without_losing_cursor_or_full_counts() {
    let entries: Vec<_> = (0..32).map(|index| json!({"sequence":100-index,"source":"automation","sourceId":Uuid::new_v4(),"projectId":null,"taskId":null,"runId":null,"kind":"decision","title":"Review","detail":"🟢".repeat(1000),"occurredAt":1000,"needsAction":true,"state":{"key":format!("ion:v2{:016x}",100-index),"read":false,"acknowledged":false,"archived":false}})).collect();
    let page: InboxPage = serde_json::from_value(json!({"revision":8,"historyRevision":100,"notificationPolicy":"all","entries":entries,"nextCursor":69,"matchingCount":100,"unreadCount":82,"needsActionCount":50,"taskHistorySyncing":true})).unwrap();
    let result = inbox_result(page, 1000).unwrap();
    assert!(result.to_string().len() <= 60000);
    assert_eq!(result["pageTruncated"], true);
    assert_eq!(result["page"]["matchingCount"], 100);
    assert_eq!(result["page"]["unreadCount"], 82);
    assert_eq!(result["page"]["needsActionCount"], 50);
    let rows = result["page"]["entries"].as_array().unwrap();
    assert!(!rows.is_empty() && rows.len() < 32);
    assert!(rows.iter().all(|row| row["detailComplete"] == false));
    assert_eq!(
        result["page"]["nextCursor"],
        rows.last().unwrap()["sequence"]
    );
}
#[test]
fn usage_transfer_keeps_unknown_costs_allowances_quality_and_totals_without_host_or_billing_references()
 {
    let id = Uuid::new_v4();
    let page: UsageLedgerPage = serde_json::from_value(json!({"revision":20,"entries":[{"sequence":20,"kind":"gpuJob","sourceId":id,"projectId":id,"taskId":id,"runId":null,"assignmentId":null,"attemptId":null,"agentId":null,"agentName":null,"provider":"nebius","model":null,"environment":"nebiusSandbox","host":"/private/host","observedAt":1000,"startedAt":null,"updatedAt":1001,"status":"uncertain","inputTokens":null,"outputTokens":null,"reservedTokens":0,"elapsedMilliseconds":null,"cost":null,"spendingAllowance":{"currency":"USD","micros":1000000}}],"nextCursor":null,"summary":{"records":100,"modelCalls":99,"inputTokens":52,"outputTokens":12,"reservedTokens":128,"unknownCalls":5,"unknownCostRecords":100,"elapsedMilliseconds":16,"costs":[]}})).unwrap();
    let mut with_cost = page.clone();
    with_cost.entries[0].cost=Some(serde_json::from_value(json!({"amount":{"currency":"USD","micros":5},"quality":"reported","finality":"provisional","origin":"private-billing-reference"})).unwrap());
    let result = usage_result(page, 1000).unwrap();
    assert_eq!(result["page"]["summary"]["records"], 100);
    assert!(result["page"]["entries"][0]["cost"].is_null());
    assert_eq!(
        result["page"]["entries"][0]["spendingAllowance"]["micros"],
        1000000
    );
    assert!(!result.to_string().contains("private/host"));
    let result = usage_result(with_cost, 1000).unwrap();
    assert!(!result.to_string().contains("private-billing"));
    assert_eq!(result["page"]["entries"][0]["cost"]["quality"], "reported");
    assert_eq!(
        result["page"]["entries"][0]["cost"]["finality"],
        "provisional"
    );
}
