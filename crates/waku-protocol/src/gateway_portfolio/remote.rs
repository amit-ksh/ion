//! Read-only remote inventory projection. Local task/cursor identity is retained;
//! cloud targets/handles and internal attempt/agent identity never cross the relay.
use super::*;
use crate::remote_control::RemoteExecutionPage;

pub fn result(page: RemoteExecutionPage, served_at: u64) -> Result<Value> {
    ensure!(
        page.entries.len() <= 32,
        "Remote gateway page exceeds 32 rows"
    );
    let entries: Vec<Value> = page.entries.into_iter().map(|row| json!({
        "kind":row.kind,"requestId":row.request_id,"rootTaskId":row.root_task_id,
        "projectId":row.project_id,"teamName":label(&row.team_name),
        "objective":row.objective.chars().take(500).collect::<String>(),
        "state":label(&row.state),"operationAvailable":row.operation_id.is_some(),
        "jobAvailable":row.job_id.is_some(),"cancellationAvailable":row.cancellation_operation_id.is_some(),
        "cleanupState":row.cleanup_state.as_deref().map(label),
        "stopRequested":row.stop_requested,"settled":row.settled,
    })).collect();
    let mut value = json!({"kind":"remoteExecutions","page":{"entries":entries,"nextCursor":page.next_cursor},"pageTruncated":false,"servedAt":served_at});
    while serde_json::to_vec(&value)?.len() > 60_000 {
        let rows = value["page"]["entries"].as_array_mut().unwrap();
        rows.pop();
        let last = rows
            .last()
            .ok_or_else(|| anyhow::anyhow!("Remote row exceeds gateway transfer bound"))?;
        let cursor = json!({"kind":last["kind"],"requestId":last["requestId"]});
        value["page"]["nextCursor"] = cursor;
        value["pageTruncated"] = true.into();
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote_control::{RemoteExecution, RemoteExecutionCursor, RemoteExecutionKind};
    fn entry() -> RemoteExecution {
        RemoteExecution {
            kind: RemoteExecutionKind::Gpu,
            request_id: Uuid::new_v4(),
            run_id: Uuid::new_v4(),
            root_task_id: Uuid::new_v4(),
            project_id: Uuid::new_v4(),
            assignment_id: Uuid::new_v4(),
            task_id: Uuid::new_v4(),
            attempt_id: Uuid::new_v4(),
            agent_id: Uuid::new_v4(),
            team_name: "Team".into(),
            objective: "Work".into(),
            state: "canceling".into(),
            remote_project: "private-cloud-project".into(),
            operation_id: Some("private-original-operation".into()),
            job_id: Some("private-job".into()),
            cancellation_operation_id: None,
            cleanup_state: Some("pending".into()),
            stop_requested: true,
            settled: false,
        }
    }
    #[test]
    fn remote_gateway_projection_omits_private_identity_and_preserves_stop_cleanup() {
        let row = entry();
        let private = [
            row.run_id,
            row.assignment_id,
            row.task_id,
            row.attempt_id,
            row.agent_id,
        ];
        let request = row.request_id;
        let root = row.root_task_id;
        let project = row.project_id;
        let value = result(
            RemoteExecutionPage {
                entries: vec![row],
                next_cursor: None,
            },
            123,
        )
        .unwrap();
        let body = value.to_string();
        for id in private {
            assert!(!body.contains(&id.to_string()));
        }
        for text in [
            "private-cloud-project",
            "private-original-operation",
            "private-job",
            "operationId",
            "jobId",
            "cancellationOperationId",
            "remoteProject",
        ] {
            assert!(!body.contains(text));
        }
        let row = &value["page"]["entries"][0];
        assert_eq!(row["requestId"], request.to_string());
        assert_eq!(row["rootTaskId"], root.to_string());
        assert_eq!(row["projectId"], project.to_string());
        assert_eq!(row["operationAvailable"], true);
        assert_eq!(row["jobAvailable"], true);
        assert_eq!(row["cancellationAvailable"], false);
        assert_eq!(row["cleanupState"], "pending");
        assert_eq!(row["stopRequested"], true);
        assert_eq!(row["settled"], false);
    }
    #[test]
    fn remote_gateway_page_bounds_labels_whole_rows_and_cursor() {
        let mut rows: Vec<_> = (0..32)
            .map(|_| {
                let mut row = entry();
                row.team_name = "語".repeat(100);
                row.objective = "語".repeat(600);
                row
            })
            .collect();
        rows.sort_by_key(|row| row.request_id);
        let value = result(
            RemoteExecutionPage {
                entries: rows.clone(),
                next_cursor: None,
            },
            123,
        )
        .unwrap();
        assert!(serde_json::to_vec(&value).unwrap().len() <= 60_000);
        let kept = value["page"]["entries"].as_array().unwrap();
        assert!(kept.len() > 0 && kept.len() < 32);
        assert_eq!(value["pageTruncated"], true);
        assert_eq!(
            value["page"]["nextCursor"],
            json!({"kind":"gpu","requestId":rows[kept.len()-1].request_id})
        );
        assert_eq!(kept[0]["teamName"].as_str().unwrap().chars().count(), 80);
        assert_eq!(kept[0]["objective"].as_str().unwrap().chars().count(), 500);
        assert!(
            result(
                RemoteExecutionPage {
                    entries: (0..33).map(|_| entry()).collect(),
                    next_cursor: None
                },
                0
            )
            .is_err()
        );
    }
    #[test]
    fn remote_gateway_empty_and_missing_handles_remain_explicit() {
        let empty = result(
            RemoteExecutionPage {
                entries: vec![],
                next_cursor: None,
            },
            0,
        )
        .unwrap();
        assert_eq!(empty["page"]["entries"], json!([]));
        assert!(empty["page"]["nextCursor"].is_null());
        let mut row = entry();
        row.operation_id = None;
        row.job_id = None;
        row.cleanup_state = None;
        let cursor = RemoteExecutionCursor {
            kind: row.kind,
            request_id: row.request_id,
        };
        let value = result(
            RemoteExecutionPage {
                entries: vec![row],
                next_cursor: Some(cursor.clone()),
            },
            0,
        )
        .unwrap();
        let row = &value["page"]["entries"][0];
        assert_eq!(row["operationAvailable"], false);
        assert_eq!(row["jobAvailable"], false);
        assert_eq!(row["settled"], false);
        assert!(row["cleanupState"].is_null());
        assert_eq!(
            value["page"]["nextCursor"],
            serde_json::to_value(cursor).unwrap()
        );
    }
    #[test]
    fn remote_gateway_query_rejects_bounds_unknown_fields_and_wrong_cursor_kind() {
        let valid = json!({"kind":"remoteExecutions","query":{"projectId":null,"kind":null,"unresolvedOnly":false,"after":null,"limit":32}});
        let query: PortfolioQuery = serde_json::from_value(valid.clone()).unwrap();
        assert!(query.validate().is_ok());
        for limit in [0, 33, 65535] {
            let mut bad = valid.clone();
            bad["query"]["limit"] = limit.into();
            let query: PortfolioQuery = serde_json::from_value(bad).unwrap();
            assert!(query.validate().is_err());
        }
        let mut bad = valid.clone();
        bad["query"]["token"] = "secret".into();
        assert!(serde_json::from_value::<PortfolioQuery>(bad).is_err());
        let mut bad = valid.clone();
        bad["query"]["after"] = json!({"kind":"model","requestId":Uuid::nil()});
        let query: PortfolioQuery = serde_json::from_value(bad).unwrap();
        assert!(query.validate().is_err());
        let mut bad = valid;
        bad["query"]["kind"] = "gpu".into();
        bad["query"]["after"] = json!({"kind":"model","requestId":Uuid::new_v4()});
        let query: PortfolioQuery = serde_json::from_value(bad).unwrap();
        assert!(query.validate().is_err());
    }
}
