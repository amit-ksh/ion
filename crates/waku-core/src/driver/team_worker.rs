//! Machine-to-machine, versioned single-completion worker contract. It has no
//! queue, filesystem/app tools, shell, delegation or owner authentication material.
use crate::driver::{
    self,
    team_remote::{WorkerRequest, WorkerResponse},
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use waku_protocol::model::ProviderKind;

pub fn execute(input: &[u8]) -> Result<Value> {
    ensure!(input.len() <= 128 * 1024, "Worker input exceeds its bound");
    let request: WorkerRequest = serde_json::from_slice(input)?;
    ensure!(
        request.version == 1
            && !request.request_id.is_nil()
            && request.system.len() <= 32 * 1024
            && request.messages.len() <= 256
            && request.tools.is_array(),
        "Unsupported worker request"
    );
    let cap = &request.capability;
    ensure!(
        (1..=32_000_000).contains(&cap.input_cap)
            && (1..=32_000_000).contains(&cap.output_cap)
            && !cap.model.is_empty()
            && cap.model.len() <= 160,
        "Invalid worker capability"
    );
    let model = &cap.model;
    let cancelled = AtomicBool::new(false);
    let reply = match cap.provider {
        ProviderKind::Nebius => driver::nebius::team::worker_completion(
            model,
            &request.system,
            &request.messages,
            request.tools.clone(),
            &cancelled,
            cap.input_cap,
            cap.output_cap,
        )?,
        ProviderKind::Codex => {
            let binary = crate::model::provider_probe(ProviderKind::Codex, None)
                .path
                .context("Codex worker executable missing")?;
            driver::team_codex::ready(&binary, model)?;
            let actual = driver::team_codex::allowance(model)?;
            ensure!(
                actual.max_input_tokens <= cap.input_cap
                    && actual.max_output_tokens <= cap.output_cap,
                "Registered Codex allowance is too small"
            );
            driver::team_codex::completion(
                &binary,
                model,
                &request.system,
                &request.messages,
                &request.tools,
                &cancelled,
            )?
        }
        ProviderKind::Claude => {
            let binary = crate::model::provider_probe(ProviderKind::Claude, None)
                .path
                .context("Claude worker executable missing")?;
            driver::team_claude::ready(&binary)?;
            let actual = driver::team_claude::allowance(model)?;
            ensure!(
                actual.max_input_tokens <= cap.input_cap
                    && actual.max_output_tokens <= cap.output_cap,
                "Registered Claude allowance is too small"
            );
            let cwd = driver::team_codex::EmptyDirectory::create("remote-claude")?;
            driver::team_claude::completion(
                &binary,
                &cwd.0,
                model,
                &request.system,
                &request.messages,
                &request.tools,
                &cancelled,
            )?
        }
        ProviderKind::Pi => driver::team_pi::worker_completion(
            model,
            &request.system,
            &request.messages,
            &request.tools,
            &cancelled,
            cap.input_cap,
            cap.output_cap,
        )?,
        ProviderKind::OpenCode => driver::team_opencode::worker_completion(
            model,
            &request.system,
            &request.messages,
            &request.tools,
            &cancelled,
            cap.input_cap,
            cap.output_cap,
        )?,
        _ => anyhow::bail!("Selected harness has no bounded worker adapter"),
    };
    let content=json!({"text":reply.text,"calls":reply.calls.into_iter().map(|c|Ok(json!({"name":c.name,"arguments":serde_json::from_str::<Value>(&c.arguments)?}))).collect::<Result<Vec<_>>>()?}).to_string();
    ensure!(
        content.len() <= 32 * 1024,
        "Worker output exceeds its bound"
    );
    Ok(serde_json::to_value(WorkerResponse {
        version: 1,
        request_id: request.request_id,
        provider: cap.provider,
        model: model.clone(),
        content,
        usage: reply.usage.context("Worker usage receipt missing")?,
    })?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> Value {
        json!({"version":1,"request_id":uuid::Uuid::new_v4(),"capability":{"provider":"amp","model":"fixture","input_cap":1000,"output_cap":100},"system":"fixture","messages":[],"tools":[]})
    }
    #[test]
    fn team_worker_rejects_invalid_contracts_before_authentication_or_execution() {
        assert!(execute(&vec![b' '; 128 * 1024 + 1]).is_err());
        for (field, value) in [
            ("version", json!(2)),
            ("request_id", json!(uuid::Uuid::nil())),
            ("tools", json!({})),
            ("extra", json!(true)),
        ] {
            let mut input = request();
            input[field] = value;
            assert!(execute(&serde_json::to_vec(&input).unwrap()).is_err());
        }
        for key in ["input_cap", "output_cap"] {
            let mut input = request();
            input["capability"][key] = json!(0);
            assert!(execute(&serde_json::to_vec(&input).unwrap()).is_err());
        }
        // The valid contract still cannot substitute a supported native harness.
        let error = execute(&serde_json::to_vec(&request()).unwrap()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Selected harness has no bounded worker adapter")
        );
    }
}
