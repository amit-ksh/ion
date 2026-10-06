//! A single-request loopback transport gate for CLI harnesses. The CLI owns auth;
//! the gate removes native tools, checks the pinned model, forbids retries and
//! forwards only to a fixed provider URL. Neither credentials nor raw events persist.
use crate::teams::ModelTokenUsage;
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

pub(super) struct Receipt {
    pub text: String,
    pub usage: ModelTokenUsage,
}
pub(super) struct Gate {
    pub url: String,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<Result<Receipt>>>,
}
impl Gate {
    pub fn responses(model: &str, upstream: &str) -> Result<Self> {
        ensure!(
            matches!(
                upstream,
                "https://api.openai.com/v1/responses"
                    | "https://chatgpt.com/backend-api/codex/responses"
            ),
            "Unsupported Codex upstream"
        );
        Self::start(model, upstream)
    }
    pub fn api(model: &str, upstream: &str) -> Result<Self> {
        ensure!(
            matches!(
                upstream,
                "https://api.openai.com/v1"
                    | "https://api.anthropic.com/v1"
                    | "https://api.tokenfactory.nebius.com/v1"
            ),
            "Unsupported OpenCode API upstream"
        );
        Self::start(model, upstream)
    }
    pub fn deepseek(model: &str) -> Result<Self> {
        Self::start(model, "https://api.deepseek.com/chat/completions")
    }
    pub fn kimi(model: &str, output: u64) -> Result<Self> {
        ensure!(
            (1..=32768).contains(&output),
            "Kimi completion limit is invalid"
        );
        Self::start_with_output_limit(
            model,
            "https://api.kimi.com/coding/v1/chat/completions",
            Some(output),
        )
    }
    pub(super) fn start(model: &str, upstream: &str) -> Result<Self> {
        Self::start_with_output_limit(model, upstream, None)
    }
    pub(super) fn start_with_output_limit(
        model: &str,
        upstream: &str,
        output_limit: Option<u64>,
    ) -> Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let prefix = format!("/ion/{}", uuid::Uuid::new_v4());
        let url = format!("http://{}{prefix}", listener.local_addr()?);
        let model = model.to_owned();
        let upstream = upstream.to_owned();
        let stop = Arc::new(AtomicBool::new(false));
        let cancelled = stop.clone();
        let worker=thread::Builder::new().name("ion-harness-gate".into()).spawn(move || {
            let deadline=Instant::now()+Duration::from_secs(100);
            loop {
                ensure!(!cancelled.load(Ordering::Acquire) && Instant::now()<deadline,"Harness gate stopped before a request");
                match listener.accept() {
                    Ok((mut connection,_)) => {
                        // One connection/request only. Dropping the listener makes retries impossible.
                        drop(listener);
                        // Windows accepted sockets inherit the listener's mode.
                        // HTTP reads must wait for data, bounded by these timeouts.
                        connection.set_nonblocking(false)?;
                        connection.set_read_timeout(Some(Duration::from_secs(8)))?;
                        connection.set_write_timeout(Some(Duration::from_secs(8)))?;
                        let result=forward(&mut connection,&model,&upstream,&prefix,output_limit);
                        if result.is_err() { let _=connection.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); }
                        return result;
                    },
                    Err(e) if e.kind()==std::io::ErrorKind::WouldBlock => thread::sleep(Duration::from_millis(20)),
                    Err(e)=>return Err(e.into())
                }
            }
        })?;
        Ok(Self {
            url,
            stop,
            worker: Some(worker),
        })
    }
    pub fn receipt(mut self) -> Result<Receipt> {
        self.stop.store(true, Ordering::Release);
        self.worker
            .take()
            .context("Harness gate already joined")?
            .join()
            .map_err(|_| anyhow::anyhow!("Harness gate failed"))?
    }
}
impl Drop for Gate {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
fn forward(
    connection: &mut TcpStream,
    model: &str,
    upstream: &str,
    prefix: &str,
    output_limit: Option<u64>,
) -> Result<Receipt> {
    let (headers, mut body) = request(connection)?;
    let first = headers
        .lines()
        .next()
        .context("Empty harness HTTP request")?;
    let route = first
        .split_whitespace()
        .nth(1)
        .and_then(|route| route.strip_prefix(prefix))
        .context("Harness transport capability is invalid")?;
    ensure!(
        first.starts_with("POST ")
            && first.ends_with(" HTTP/1.1")
            && matches!(
                route,
                "/responses"
                    | "/v1/responses"
                    | "/chat/completions"
                    | "/v1/chat/completions"
                    | "/messages"
                    | "/v1/messages"
            ),
        "Unsupported harness route"
    );
    ensure!(
        body["model"].as_str() == Some(model),
        "Harness changed the pinned model"
    );
    if let Some(limit) = output_limit {
        ensure!(
            route.ends_with("/chat/completions")
                && body["max_completion_tokens"]
                    .as_u64()
                    .is_some_and(|cap| cap > 0 && cap <= limit)
                && body.get("max_tokens").is_none(),
            "Harness completion cap changed; no provider request was sent"
        );
    }
    let messages = route.ends_with("/messages");
    let chat = route.ends_with("/chat/completions");
    body["tools"] = json!([]);
    body["tool_choice"] = if messages {
        json!({"type":"none"})
    } else {
        json!("none")
    };
    body.as_object_mut()
        .context("Harness request must be an object")?
        .remove("parallel_tool_calls");
    body["stream"] = json!(true);
    if chat {
        body["stream_options"] = json!({"include_usage":true});
    }
    let upstream = if upstream.ends_with("/responses") {
        ensure!(!messages && !chat, "Codex route changed");
        upstream.to_owned()
    } else if upstream.ends_with("/chat/completions") {
        ensure!(chat && !messages, "Chat route changed");
        upstream.to_owned()
    } else {
        ensure!(
            messages == upstream.starts_with("https://api.anthropic.com/"),
            "Provider protocol changed"
        );
        format!(
            "{upstream}/{}",
            if messages {
                "messages"
            } else if chat {
                "chat/completions"
            } else {
                "responses"
            }
        )
    };
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()?;
    let mut request = client.post(&upstream).json(&body);
    let mut authenticated = false;
    for line in headers.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let native_kimi_header = upstream == "https://api.kimi.com/coding/v1/chat/completions"
            && matches!(
                name.to_ascii_lowercase().as_str(),
                "user-agent"
                    | "x-msh-platform"
                    | "x-msh-version"
                    | "x-msh-device-name"
                    | "x-msh-device-model"
                    | "x-msh-os-version"
                    | "x-msh-device-id"
            );
        if native_kimi_header
            || matches!(
                name.to_ascii_lowercase().as_str(),
                "authorization"
                    | "x-api-key"
                    | "anthropic-version"
                    | "anthropic-beta"
                    | "chatgpt-account-id"
                    | "openai-organization"
                    | "openai-project"
                    | "openai-beta"
            )
        {
            if name.eq_ignore_ascii_case("authorization") || name.eq_ignore_ascii_case("x-api-key")
            {
                ensure!(
                    !authenticated
                        && (!name.eq_ignore_ascii_case("authorization")
                            || value.trim().starts_with("Bearer ")),
                    "Invalid harness authentication"
                );
                authenticated = true;
            }
            request = request.header(name, value.trim());
        }
    }
    ensure!(authenticated, "Harness did not supply authentication");
    let response = request.send().context("Harness provider request failed")?;
    ensure!(
        response.status().is_success(),
        "Harness provider rejected the request (HTTP {})",
        response.status()
    );
    let mut bytes = Vec::new();
    response.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Harness provider response exceeded its limit"
    );
    // Validate before forwarding: native tool calls from even a malformed provider
    // response can never reach the CLI's tool dispatcher.
    let receipt = if messages {
        parse_messages(&bytes, model)?
    } else if chat {
        parse_chat(&bytes, model)?
    } else {
        parse_responses(&bytes, model)?
    };
    write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    connection.write_all(&bytes)?;
    Ok(receipt)
}
fn request(connection: &mut TcpStream) -> Result<(String, Value)> {
    let mut data = Vec::new();
    let mut chunk = [0; 4096];
    let offset = loop {
        let read = connection.read(&mut chunk)?;
        ensure!(read > 0, "Harness closed an incomplete request");
        data.extend_from_slice(&chunk[..read]);
        if let Some(offset) = data.windows(4).position(|v| v == b"\r\n\r\n") {
            break offset + 4;
        }
        ensure!(
            data.len() <= 16 * 1024,
            "Harness headers exceeded their limit"
        );
    };
    ensure!(offset <= 16 * 1024, "Harness headers exceeded their limit");
    let headers = std::str::from_utf8(&data[..offset])?.to_owned();
    let mut lengths = headers
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("content-length"));
    let length: usize = lengths
        .next()
        .context("Harness requires Content-Length")?
        .1
        .trim()
        .parse()?;
    ensure!(
        lengths.next().is_none() && length <= 256 * 1024,
        "Invalid harness request length"
    );
    ensure!(
        !headers
            .lines()
            .any(|l| l.to_ascii_lowercase().starts_with("transfer-encoding:")),
        "Chunked harness requests are unsupported"
    );
    while data.len() < offset + length {
        let read = connection.read(&mut chunk)?;
        ensure!(read > 0, "Incomplete harness body");
        data.extend_from_slice(&chunk[..read]);
    }
    ensure!(
        data.len() == offset + length,
        "Pipelined harness requests are forbidden"
    );
    Ok((headers, serde_json::from_slice(&data[offset..])?))
}
fn parse_responses(bytes: &[u8], model: &str) -> Result<Receipt> {
    let stream = std::str::from_utf8(bytes)?;
    let mut final_response = None;
    for line in stream.lines().filter_map(|l| l.strip_prefix("data:")) {
        if line.trim() == "[DONE]" {
            continue;
        }
        let event: Value = serde_json::from_str(line.trim())?;
        let kind = event["type"]
            .as_str()
            .context("Provider event type missing")?;
        ensure!(
            !kind.contains("function_call") && !kind.contains("tool_call"),
            "Native harness tool calls are forbidden"
        );
        if let Some(item) = event.get("item") {
            ensure!(
                matches!(item["type"].as_str(), Some("message" | "reasoning")),
                "Unexpected native harness item"
            );
        }
        ensure!(
            !matches!(kind, "error" | "response.failed" | "response.incomplete"),
            "Provider did not complete the bounded request"
        );
        ensure!(
            matches!(
                kind,
                "response.created"
                    | "response.in_progress"
                    | "response.queued"
                    | "response.completed"
                    | "response.output_item.added"
                    | "response.output_item.done"
                    | "response.content_part.added"
                    | "response.content_part.done"
                    | "response.output_text.delta"
                    | "response.output_text.done"
                    | "response.output_text.annotation.added"
                    | "response.reasoning_summary_part.added"
                    | "response.reasoning_summary_part.done"
                    | "response.reasoning_summary_text.delta"
                    | "response.reasoning_summary_text.done"
                    | "response.reasoning_text.delta"
                    | "response.reasoning_text.done"
            ),
            "Unexpected provider control event"
        );
        if kind == "response.completed" {
            ensure!(final_response.is_none(), "Duplicate provider completion");
            final_response = Some(event["response"].clone());
        }
    }
    let response = final_response.context("Provider usage/completion receipt missing")?;
    ensure!(
        response["status"] == "completed" && response["model"].as_str() == Some(model),
        "Provider changed model or did not finish"
    );
    let output = response["output"]
        .as_array()
        .context("Provider output missing")?;
    let mut text = String::new();
    for item in output {
        match item["type"].as_str() {
            Some("reasoning") => {}
            Some("message") => {
                ensure!(item["role"] == "assistant", "Unexpected response role");
                for content in item["content"]
                    .as_array()
                    .context("Provider message content missing")?
                {
                    ensure!(
                        content["type"] == "output_text",
                        "Unexpected provider content"
                    );
                    text.push_str(content["text"].as_str().context("Provider text missing")?);
                }
            }
            _ => anyhow::bail!("Native harness effects are forbidden"),
        }
    }
    let token = |key: &str| {
        response["usage"][key]
            .as_u64()
            .filter(|n| *n <= 32_000_000)
            .context("Provider token usage missing or invalid")
    };
    Ok(Receipt {
        text,
        usage: ModelTokenUsage {
            input_tokens: token("input_tokens")?,
            output_tokens: token("output_tokens")?,
        },
    })
}

fn usage_token(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .filter(|n| *n <= 32_000_000)
        .context("Provider token usage missing or invalid")
}
fn parse_chat(bytes: &[u8], model: &str) -> Result<Receipt> {
    let mut text = String::new();
    let mut usage = None;
    let mut finished = false;
    for line in std::str::from_utf8(bytes)?
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
    {
        if line.trim() == "[DONE]" {
            continue;
        }
        let v: Value = serde_json::from_str(line.trim())?;
        ensure!(
            v["model"].as_str() == Some(model) && v.get("error").is_none(),
            "Provider model/error receipt changed"
        );
        for choice in v["choices"]
            .as_array()
            .context("Provider choices missing")?
        {
            ensure!(choice["index"] == 0 && !finished, "Extra provider output");
            let delta = &choice["delta"];
            ensure!(
                delta.get("tool_calls").is_none() && delta.get("function_call").is_none(),
                "Native harness tools are forbidden"
            );
            if let Some(value) = delta["content"].as_str() {
                text.push_str(value);
            }
            if let Some(reason) = choice["finish_reason"].as_str() {
                ensure!(reason == "stop", "Provider completion interrupted");
                finished = true;
            }
        }
        if v["usage"].is_object() {
            ensure!(usage.is_none(), "Duplicate provider usage");
            usage = Some(ModelTokenUsage {
                input_tokens: usage_token(&v["usage"], "prompt_tokens")?,
                output_tokens: usage_token(&v["usage"], "completion_tokens")?,
            });
        }
    }
    ensure!(finished, "Provider completion receipt missing");
    Ok(Receipt {
        text,
        usage: usage.context("Provider usage receipt missing")?,
    })
}
fn parse_messages(bytes: &[u8], model: &str) -> Result<Receipt> {
    let mut text = String::new();
    let mut input = None;
    let mut output = None;
    let mut ended = false;
    let mut finished = false;
    for line in std::str::from_utf8(bytes)?
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
    {
        let v: Value = serde_json::from_str(line.trim())?;
        match v["type"].as_str().context("Provider event missing")? {
            "message_start" => {
                ensure!(
                    input.is_none() && v["message"]["model"].as_str() == Some(model),
                    "Provider model identity changed"
                );
                let usage = &v["message"]["usage"];
                let count = usage_token(usage, "input_tokens")?;
                let cache = |key: &str| {
                    if usage.get(key).is_none() {
                        Ok(0)
                    } else {
                        usage_token(usage, key)
                    }
                };
                input = Some(
                    count
                        .checked_add(cache("cache_read_input_tokens")?)
                        .and_then(|n| n.checked_add(cache("cache_creation_input_tokens").ok()?))
                        .context("Provider token overflow")?,
                );
            }
            "content_block_start" => {
                ensure!(
                    matches!(
                        v["content_block"]["type"].as_str(),
                        Some("text" | "thinking" | "redacted_thinking")
                    ),
                    "Native harness tools are forbidden"
                );
            }
            "content_block_delta" => match v["delta"]["type"].as_str() {
                Some("text_delta") => text.push_str(
                    v["delta"]["text"]
                        .as_str()
                        .context("Provider text missing")?,
                ),
                Some("thinking_delta" | "signature_delta") => {}
                _ => anyhow::bail!("Native harness tool delta is forbidden"),
            },
            "message_delta" => {
                ensure!(
                    !ended && v["delta"]["stop_reason"] == "end_turn",
                    "Provider completion interrupted"
                );
                output = Some(usage_token(&v["usage"], "output_tokens")?);
                ended = true;
            }
            "message_stop" => {
                ensure!(ended && !finished, "Invalid provider completion sequence");
                finished = true;
            }
            "content_block_stop" | "ping" => {}
            _ => anyhow::bail!("Unexpected provider event"),
        }
    }
    ensure!(finished, "Provider completion receipt missing");
    Ok(Receipt {
        text,
        usage: ModelTokenUsage {
            input_tokens: input.context("Provider input usage missing")?,
            output_tokens: output.context("Provider output usage missing")?,
        },
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn event(model: &str) -> Vec<u8> {
        format!("data: {}\n\n",json!({"type":"response.completed","response":{"status":"completed","model":model,"output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"{}"}]}],"usage":{"input_tokens":100,"output_tokens":5}}})).into_bytes()
    }
    #[test]
    fn team_gate_checks_provider_receipts_and_never_forwards_native_tools() {
        assert_eq!(
            parse_responses(&event("pinned"), "pinned")
                .unwrap()
                .usage
                .input_tokens,
            100
        );
        assert!(parse_responses(&event("replacement"), "pinned").is_err());
        assert!(parse_responses(b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\"}}\n\n","pinned").is_err());
        let mut twice = event("pinned");
        twice.extend(event("pinned"));
        assert!(parse_responses(&twice, "pinned").is_err());
        assert!(
            parse_responses(b"data: {\"type\":\"response.incomplete\"}\n\n", "pinned").is_err()
        );
    }
}

#[cfg(test)]
mod transport_tests {
    use super::*;
    use crate::driver::team_test_http::{Server, responses};
    #[test]
    fn team_gate_kimi_rejects_missing_changed_or_ambiguous_output_caps_before_billing() {
        for cap in [json!(null), json!(0), json!(32769), json!("32768")] {
            let gate = Gate::start_with_output_limit(
                "k3",
                "http://127.0.0.1:1/chat/completions",
                Some(32768),
            )
            .unwrap();
            let response = reqwest::blocking::Client::new()
                .post(format!("{}/chat/completions", gate.url))
                .bearer_auth("fixture-only")
                .json(&json!({"model":"k3","max_completion_tokens":cap}))
                .send()
                .unwrap();
            assert_eq!(response.status(), 502);
            let error = gate.receipt().err().unwrap().to_string();
            assert!(error.contains("completion cap changed"), "{error}");
        }
        let gate =
            Gate::start_with_output_limit("k3", "http://127.0.0.1:1/chat/completions", Some(32768))
                .unwrap();
        let response = reqwest::blocking::Client::new()
            .post(format!("{}/chat/completions", gate.url))
            .bearer_auth("fixture-only")
            .json(&json!({"model":"k3","max_completion_tokens":32768,"max_tokens":99999}))
            .send()
            .unwrap();
        assert_eq!(response.status(), 502);
        assert!(
            gate.receipt()
                .err()
                .unwrap()
                .to_string()
                .contains("completion cap changed")
        );
    }
    #[test]
    fn team_gate_waits_for_data_after_accepting_a_nonblocking_listener_socket() {
        let server = Server::new(vec![(
            200,
            responses("pinned", r#"{"text":"ready","calls":[]}"#),
        )]);
        let gate = Gate::start("pinned", &format!("{}/responses", server.url)).unwrap();
        let url = url::Url::parse(&gate.url).unwrap();
        let mut connection = TcpStream::connect(("127.0.0.1", url.port().unwrap())).unwrap();
        connection
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        // Ensure the gate accepts before a request byte arrives. A nonblocking
        // accepted socket would return WouldBlock and close this connection.
        thread::sleep(Duration::from_millis(150));
        let body = json!({"model":"pinned","input":"fixture"}).to_string();
        write!(connection,"POST {}/responses HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer fixture-only\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",url.path(),body.len()).unwrap();
        let mut output = String::new();
        connection.read_to_string(&mut output).unwrap();
        assert!(output.starts_with("HTTP/1.1 200 OK"));
        assert_eq!(gate.receipt().unwrap().usage.input_tokens, 100);
        assert_eq!(server.finish().len(), 1);
    }
    #[test]
    fn team_gate_http_strips_effects_auth_headers_are_private_and_retry_cannot_bill() {
        let server = Server::new(vec![(
            200,
            responses("pinned", r#"{"text":"ready","calls":[]}"#),
        )]);
        let gate = Gate::start("pinned", &format!("{}/responses", server.url)).unwrap();
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/responses", gate.url);
        let response=client.post(&url).bearer_auth("fixture-only-not-a-key").header("X-Owner-Secret","never-forward")
            .json(&json!({"model":"pinned","tools":[{"type":"function","name":"shell"}],"tool_choice":"required","parallel_tool_calls":true})).send().unwrap();
        assert_eq!(response.status(), 200);
        assert!(
            client
                .post(&url)
                .json(&json!({"model":"pinned"}))
                .send()
                .is_err()
        );
        let receipt = gate.receipt().unwrap();
        assert_eq!(receipt.usage.output_tokens, 5);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["tools"], json!([]));
        assert_eq!(requests[0].body["tool_choice"], "none");
        assert!(requests[0].body.get("parallel_tool_calls").is_none());
        assert!(!requests[0].headers.contains("never-forward"));
    }
    #[test]
    fn team_gate_http_rejects_native_effect_before_cli_receives_it() {
        let server=Server::new(vec![(200,b"data: {\"type\":\"response.output_item.added\",\"item\":{\"type\":\"function_call\"}}\n\n".to_vec())]);
        let gate = Gate::start("pinned", &format!("{}/responses", server.url)).unwrap();
        let response = reqwest::blocking::Client::new()
            .post(format!("{}/responses", gate.url))
            .bearer_auth("fixture")
            .json(&json!({"model":"pinned"}))
            .send()
            .unwrap();
        assert_eq!(response.status(), 502);
        assert!(response.text().unwrap().is_empty());
        assert!(gate.receipt().is_err());
        assert_eq!(server.finish().len(), 1);
    }
}
