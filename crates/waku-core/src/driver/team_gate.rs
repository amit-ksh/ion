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
#[derive(Clone)]
struct BoundedInput {
    task: String,
    input: u64,
    output: u64,
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
    /// Private native clients are given exactly one owner-authored request.
    /// Native context, tool schemas and auxiliary prompts never leave this gate.
    pub fn bounded_api(
        model: &str,
        upstream: &str,
        input: u64,
        output: u64,
        task: &str,
    ) -> Result<Self> {
        ensure!(
            matches!(
                upstream,
                "https://api.openai.com/v1"
                    | "https://api.anthropic.com/v1"
                    | "https://api.tokenfactory.nebius.com/v1"
            ),
            "Unsupported bounded native API origin"
        );
        Self::start_bounded(model, upstream, input, output, task)
    }
    pub(super) fn start_bounded(
        model: &str,
        upstream: &str,
        input: u64,
        output: u64,
        task: &str,
    ) -> Result<Self> {
        ensure!(
            (1..=32_000_000).contains(&input) && (1..=32_000_000).contains(&output),
            "Invalid native reservation"
        );
        // Byte-level upper bound plus framing; the canonical request contains
        // no images, remote context, history references or native tool schemas.
        ensure!(
            task.len() <= 128 * 1024
                && task.len() + super::team_completion::INSTRUCTIONS.len() + 1024 <= input as usize,
            "Native input exceeds its reservation"
        );
        Self::spawn(
            model,
            upstream,
            None,
            None,
            Some(BoundedInput {
                task: task.into(),
                input,
                output,
            }),
        )
    }
    pub fn gemini(model: &str, input: u64, output: u64, task: &str) -> Result<Self> {
        ensure!(
            model == "gemini-3.1-pro-preview",
            "Unsupported native Gemini model"
        );
        Self::start_bounded(
            model,
            &format!(
                "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"
            ),
            input,
            output,
            task,
        )
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
    pub fn grok(model: &str, output: u64, task: &str) -> Result<Self> {
        ensure!(
            (1..=32768).contains(&output),
            "Grok completion limit is invalid"
        );
        Self::start_with_prompt(
            model,
            "https://api.x.ai/v1/responses",
            Some(output),
            Some(task.to_owned()),
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
        Self::start_with_prompt(model, upstream, output_limit, None)
    }
    pub(super) fn start_with_prompt(
        model: &str,
        upstream: &str,
        output_limit: Option<u64>,
        task: Option<String>,
    ) -> Result<Self> {
        Self::spawn(model, upstream, output_limit, task, None)
    }
    fn spawn(
        model: &str,
        upstream: &str,
        output_limit: Option<u64>,
        task: Option<String>,
        bounded: Option<BoundedInput>,
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
            let mut auxiliary_rejections = 0;
            loop {
                ensure!(!cancelled.load(Ordering::Acquire) && Instant::now()<deadline,"Harness gate stopped before a request");
                match listener.accept() {
                    Ok((mut connection,_)) => {
                        // Only one upstream request. Grok's unreserved title calls
                        // are rejected locally before the saved prompt is admitted.
                        // Windows accepted sockets inherit the listener's mode.
                        // HTTP reads must wait for data, bounded by these timeouts.
                        connection.set_nonblocking(false)?;
                        connection.set_read_timeout(Some(Duration::from_secs(8)))?;
                        connection.set_write_timeout(Some(Duration::from_secs(8)))?;
                        let result=forward(&mut connection,&model,&upstream,&prefix,output_limit,task.as_deref(),bounded.as_ref());
                        match result {
                            Ok(Some(receipt)) => return Ok(receipt),
                            Ok(None) => {
                                auxiliary_rejections += 1;
                                ensure!(auxiliary_rejections <= 4, "Too many unreserved native auxiliary calls");
                            },
                            Err(error) => {
                                let _=connection.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                                return Err(error);
                            }
                        }
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
    task: Option<&str>,
    bounded: Option<&BoundedInput>,
) -> Result<Option<Receipt>> {
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
    if upstream.contains(":streamGenerateContent?alt=sse") {
        ensure!(
            first.starts_with("POST ") && first.ends_with(" HTTP/1.1"),
            "Unsupported native Gemini request"
        );
        return forward_gemini(
            connection,
            &headers,
            &body,
            model,
            upstream,
            route,
            bounded.context("Gemini reservation missing")?,
        );
    }
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
    if let Some(contract) = bounded {
        let rows = if route.ends_with("/responses") {
            &body["input"]
        } else {
            &body["messages"]
        };
        let contains_task = rows.as_array().is_some_and(|rows| {
            rows.iter().any(|row| {
                row["role"] == "user"
                    && (row["content"].as_str() == Some(&contract.task)
                        || row["content"].as_array().is_some_and(|parts| {
                            parts
                                .iter()
                                .any(|part| part["text"].as_str() == Some(&contract.task))
                        }))
            })
        });
        ensure!(
            contains_task,
            "Native client changed the saved Teams prompt; no provider request was sent"
        );
        ensure!(
            body["model"].as_str() == Some(model),
            "Harness changed the pinned model"
        );
        let system = super::team_completion::INSTRUCTIONS;
        body = if route.ends_with("/responses") {
            json!({"model":model,"input":[{"role":"system","content":system},{"role":"user","content":contract.task}],"max_output_tokens":contract.output,"store":false})
        } else if route.ends_with("/messages") {
            json!({"model":model,"system":system,"messages":[{"role":"user","content":contract.task}],"max_tokens":contract.output})
        } else {
            json!({"model":model,"messages":[{"role":"system","content":system},{"role":"user","content":contract.task}],"max_tokens":contract.output})
        };
    }
    if let Some(task) = task {
        ensure!(
            route.ends_with("/responses"),
            "Grok native protocol changed"
        );
        if !grok_prompt_matches(&body, task) {
            // No auth, prompt, output or provider request leaves this loopback gate.
            let error = br#"{"error":{"message":"Ion blocked an unreserved auxiliary call"}}"#;
            write!(
                connection,
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                error.len()
            )?;
            connection.write_all(error)?;
            return Ok(None);
        }
        ensure!(
            [
                "instructions",
                "previous_response_id",
                "conversation",
                "prompt"
            ]
            .iter()
            .all(|k| body.get(k).is_none()),
            "Grok changed the saved request context"
        );
        // Native startup context and bundled workflow reminders are not part of
        // the saved Teams input. Only the exact owner instructions/input leave.
        body["input"] = json!([
            {"role":"system","content":super::team_completion::INSTRUCTIONS},
            {"role":"user","content":task}
        ]);
    }
    ensure!(
        body["model"].as_str() == Some(model),
        "Harness changed the pinned model"
    );
    if let Some(limit) = output_limit {
        ensure!(
            if route.ends_with("/responses") {
                task.is_some()
                    && body["max_output_tokens"].as_u64() == Some(limit)
                    && body.get("max_tokens").is_none()
                    && body.get("max_completion_tokens").is_none()
            } else {
                route.ends_with("/chat/completions")
                    && body["max_completion_tokens"]
                        .as_u64()
                        .is_some_and(|cap| cap > 0 && cap <= limit)
                    && body.get("max_tokens").is_none()
            },
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
    if task.is_some() {
        body["store"] = json!(false);
    }
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
    if let Some(contract) = bounded {
        ensure!(
            receipt.usage.input_tokens <= contract.input
                && receipt.usage.output_tokens <= contract.output,
            "Provider usage exceeded the reserved native request"
        );
    }
    write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    connection.write_all(&bytes)?;
    Ok(Some(receipt))
}
fn forward_gemini(
    connection: &mut TcpStream,
    headers: &str,
    native: &Value,
    model: &str,
    upstream: &str,
    route: &str,
    contract: &BoundedInput,
) -> Result<Option<Receipt>> {
    let matches = native["contents"].as_array().is_some_and(|rows| {
        rows.iter().any(|row| {
            row["role"] == "user"
                && row["parts"].as_array().is_some_and(|parts| {
                    parts.iter().any(|part| {
                        part["text"]
                            .as_str()
                            .is_some_and(|text| text.contains(&contract.task))
                    })
                })
        })
    }) && native["systemInstruction"]["parts"]
        .as_array()
        .is_some_and(|parts| {
            parts.iter().any(|part| {
                part["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(super::team_completion::INSTRUCTIONS))
            })
        });
    if !matches {
        let error = br#"{"error":{"code":400,"message":"Ion blocked an unreserved auxiliary call","status":"INVALID_ARGUMENT"}}"#;
        write!(
            connection,
            "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            error.len()
        )?;
        connection.write_all(error)?;
        return Ok(None);
    }
    ensure!(
        route == format!("/v1beta/models/{model}:streamGenerateContent?alt=sse"),
        "Antigravity changed the pinned API model"
    );
    let thinking = &native["generationConfig"]["thinkingConfig"];
    ensure!(
        thinking["thinkingBudget"]
            .as_i64()
            .is_some_and(|n| n == -1 || (0..=contract.output as i64).contains(&n)),
        "Native thinking budget changed"
    );
    let body = json!({"contents":[{"role":"user","parts":[{"text":contract.task}]}],"systemInstruction":{"parts":[{"text":super::team_completion::INSTRUCTIONS}]},"generationConfig":{"maxOutputTokens":contract.output,"thinkingConfig":thinking}});
    let key = headers
        .lines()
        .filter_map(|l| l.split_once(':'))
        .filter(|(name, _)| name.eq_ignore_ascii_case("x-goog-api-key"))
        .map(|(_, value)| value.trim())
        .collect::<Vec<_>>();
    ensure!(
        key.len() == 1 && !key[0].is_empty(),
        "Native Gemini authentication missing or ambiguous"
    );
    let client = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(90))
        .build()?;
    let response = client
        .post(upstream)
        .header("x-goog-api-key", key[0])
        .json(&body)
        .send()
        .context("Native Gemini provider request failed")?;
    ensure!(
        response.status().is_success(),
        "Native Gemini provider rejected request (HTTP {})",
        response.status()
    );
    let mut bytes = Vec::new();
    response.take(2 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 2 * 1024 * 1024,
        "Native Gemini response exceeded its bound"
    );
    let receipt = parse_gemini(&bytes, model)?;
    ensure!(
        receipt.usage.input_tokens <= contract.input
            && receipt.usage.output_tokens <= contract.output,
        "Native Gemini usage exceeded reservation"
    );
    write!(
        connection,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )?;
    connection.write_all(&bytes)?;
    Ok(Some(receipt))
}
fn parse_gemini(bytes: &[u8], model: &str) -> Result<Receipt> {
    let mut text = String::new();
    let mut finished = false;
    let mut usage = None;
    for line in std::str::from_utf8(bytes)?
        .lines()
        .filter_map(|l| l.strip_prefix("data:"))
    {
        let value: Value = serde_json::from_str(line.trim())?;
        ensure!(
            value.get("error").is_none() && value["modelVersion"].as_str() == Some(model),
            "Native Gemini model/error receipt changed"
        );
        for candidate in value["candidates"]
            .as_array()
            .context("Gemini candidates missing")?
        {
            ensure!(
                !finished && candidate["index"].as_u64() == Some(0),
                "Extra Gemini completion"
            );
            if let Some(parts) = candidate["content"]["parts"].as_array() {
                for part in parts {
                    ensure!(
                        part.get("functionCall").is_none()
                            && part.get("executableCode").is_none()
                            && part.get("codeExecutionResult").is_none()
                            && part.get("inlineData").is_none()
                            && part.get("fileData").is_none(),
                        "Native Gemini effects are forbidden"
                    );
                    let content = part["text"].as_str().context("Unexpected Gemini content")?;
                    if part["thought"] != true {
                        text.push_str(content);
                    }
                }
            }
            if let Some(reason) = candidate["finishReason"].as_str() {
                ensure!(reason == "STOP", "Gemini completion did not finish");
                finished = true;
            }
        }
        if let Some(meta) = value.get("usageMetadata") {
            let input = usage_token(meta, "promptTokenCount")?;
            let total = usage_token(meta, "totalTokenCount")?;
            let output = total
                .checked_sub(input)
                .context("Gemini usage is invalid")?;
            let optional_count = |key| -> Result<u64> {
                match meta.get(key) {
                    None => Ok(0),
                    Some(value) => value.as_u64().context("Gemini usage count is invalid"),
                }
            };
            let visible = optional_count("candidatesTokenCount")?;
            let thoughts = optional_count("thoughtsTokenCount")?;
            ensure!(
                visible.checked_add(thoughts) == Some(output)
                    && optional_count("toolUsePromptTokenCount")? == 0,
                "Gemini usage includes unknown effects or counts"
            );
            if finished {
                usage = Some(ModelTokenUsage {
                    input_tokens: input,
                    output_tokens: output,
                });
            }
        }
    }
    ensure!(finished, "Gemini completion receipt missing");
    Ok(Receipt {
        text,
        usage: usage.context("Gemini usage missing")?,
    })
}

fn grok_prompt_matches(body: &Value, task: &str) -> bool {
    let Some(input) = body["input"].as_array() else {
        return false;
    };
    let text = |row: &Value| -> Option<String> {
        if let Some(s) = row["content"].as_str() {
            return Some(s.to_owned());
        }
        let parts = row["content"].as_array()?;
        let mut result = String::new();
        for part in parts {
            if !matches!(part["type"].as_str(), Some("input_text" | "text")) {
                return None;
            }
            result.push_str(part["text"].as_str()?);
        }
        Some(result)
    };
    input.iter().any(|r| {
        r["role"] == "system" && text(r).as_deref() == Some(super::team_completion::INSTRUCTIONS)
    }) && input
        .iter()
        .any(|r| r["role"] == "user" && text(r).is_some_and(|s| s.contains(task)))
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
    fn team_gate_grok_denies_auxiliary_calls_without_upstream_or_prompt_substitution() {
        let task = r#"{"instructions":"literal @secret"}"#;
        let gate = Gate::start_with_prompt(
            "grok-4.6",
            "http://127.0.0.1:1/responses",
            Some(32768),
            Some(task.into()),
        )
        .unwrap();
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        let url = format!("{}/responses", gate.url);
        let auxiliary = json!({"model":"grok-4.6","max_output_tokens":100,"input":[{"role":"system","content":"Generate a session title"},{"role":"user","content":task}]});
        for _ in 0..4 {
            assert_eq!(
                client
                    .post(&url)
                    .bearer_auth("fixture-only")
                    .json(&auxiliary)
                    .send()
                    .unwrap()
                    .status(),
                400
            );
        }
        // A main request with a different prompt is also rejected without billing.
        let altered = json!({"model":"grok-4.6","max_output_tokens":32768,"input":[{"role":"system","content":super::super::team_completion::INSTRUCTIONS},{"role":"user","content":"replacement task"}]});
        assert_eq!(
            client
                .post(&url)
                .bearer_auth("fixture-only")
                .json(&altered)
                .send()
                .unwrap()
                .status(),
            400
        );
        assert!(gate.receipt().is_err());
        assert!(client.post(url).json(&altered).send().is_err());
    }
    #[test]
    fn team_gate_grok_requires_exact_model_and_output_cap_before_billing() {
        let task = r#"{"instructions":"saved"}"#;
        for (model, cap, other) in [
            ("replacement", json!(32768), false),
            ("grok-4.6", json!(32769), false),
            ("grok-4.6", json!(100), false),
            ("grok-4.6", json!("32768"), false),
            ("grok-4.6", json!(32768), true),
        ] {
            let gate = Gate::start_with_prompt(
                "grok-4.6",
                "http://127.0.0.1:1/responses",
                Some(32768),
                Some(task.into()),
            )
            .unwrap();
            let mut body = json!({"model":model,"max_output_tokens":cap,"input":[{"role":"system","content":super::super::team_completion::INSTRUCTIONS},{"role":"user","content":task}]});
            if other {
                body["max_tokens"] = json!(1);
            }
            let reply = reqwest::blocking::Client::new()
                .post(format!("{}/responses", gate.url))
                .bearer_auth("fixture-only")
                .json(&body)
                .send()
                .unwrap();
            assert_eq!(reply.status(), 502);
            assert!(gate.receipt().err().unwrap().to_string().contains(
                if model == "replacement" {
                    "pinned model"
                } else {
                    "cap changed"
                }
            ));
        }
    }
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
    fn gemini_data() -> Value {
        json!({"modelVersion":"gemini-3.1-pro-preview","candidates":[{"index":0,"content":{"role":"model","parts":[{"text":"hidden","thought":true},{"text":"ready"}]},"finishReason":"STOP"}],"usageMetadata":{"promptTokenCount":100,"candidatesTokenCount":5,"thoughtsTokenCount":2,"totalTokenCount":107}})
    }
    fn gemini_frame(value: &Value) -> Vec<u8> {
        format!("data: {value}\n\n").into_bytes()
    }
    #[test]
    fn team_gate_gemini_accounts_for_thoughts_and_rejects_native_effects() {
        let value = gemini_data();
        let receipt = parse_gemini(&gemini_frame(&value), "gemini-3.1-pro-preview").unwrap();
        assert_eq!(receipt.text, "ready");
        assert_eq!(receipt.usage.output_tokens, 7);
        for field in [
            "functionCall",
            "executableCode",
            "codeExecutionResult",
            "fileData",
            "inlineData",
        ] {
            let mut changed = value.clone();
            changed["candidates"][0]["content"]["parts"][1][field] = json!({});
            assert!(parse_gemini(&gemini_frame(&changed), "gemini-3.1-pro-preview").is_err());
        }
        for field in [
            "candidatesTokenCount",
            "thoughtsTokenCount",
            "toolUsePromptTokenCount",
        ] {
            let mut invalid = value.clone();
            invalid["usageMetadata"][field] = json!("0");
            assert!(parse_gemini(&gemini_frame(&invalid), "gemini-3.1-pro-preview").is_err());
        }
        let mut partial = value.clone();
        partial["candidates"][0]
            .as_object_mut()
            .unwrap()
            .remove("finishReason");
        let mut final_frame = value.clone();
        final_frame.as_object_mut().unwrap().remove("usageMetadata");
        let mut incomplete = gemini_frame(&partial);
        incomplete.extend(gemini_frame(&final_frame));
        assert!(parse_gemini(&incomplete, "gemini-3.1-pro-preview").is_err());
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove("usageMetadata");
        assert!(parse_gemini(&gemini_frame(&missing), "gemini-3.1-pro-preview").is_err());
        let mut wrong = value.clone();
        wrong["modelVersion"] = json!("replacement");
        assert!(parse_gemini(&gemini_frame(&wrong), "gemini-3.1-pro-preview").is_err());
        let mut usage = value.clone();
        usage["usageMetadata"]["totalTokenCount"] = json!(108);
        assert!(parse_gemini(&gemini_frame(&usage), "gemini-3.1-pro-preview").is_err());
    }
    #[test]
    fn team_gate_bounded_request_replaces_native_context_and_clamps_before_send() {
        let server = Server::new(vec![(200, responses("pinned", "ready"))]);
        let task = "saved task";
        let gate = Gate::start_bounded(
            "pinned",
            &format!("{}/responses", server.url),
            8192,
            16,
            task,
        )
        .unwrap();
        let url = format!("{}/responses", gate.url);
        let client = reqwest::blocking::Client::new();
        let reply=client.post(&url).bearer_auth("fixture-only").json(&json!({"model":"pinned","max_output_tokens":99999,"previous_response_id":"native-history","input":[{"role":"system","content":"native expansion"},{"role":"user","content":task}],"tools":[{"type":"web_search"}]})).send().unwrap();
        assert_eq!(reply.status(), 200);
        assert_eq!(gate.receipt().unwrap().usage.output_tokens, 5);
        assert!(client.post(url).send().is_err());
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["max_output_tokens"], 16);
        assert!(requests[0].body.get("previous_response_id").is_none());
        assert_eq!(requests[0].body["input"].as_array().unwrap().len(), 2);
        assert_eq!(requests[0].body["tools"], json!([]));
    }
    #[test]
    fn team_gate_bounded_rejects_changed_identity_and_over_reservation_usage() {
        for (model, task) in [("changed", "saved"), ("pinned", "changed")] {
            let gate =
                Gate::start_bounded("pinned", "http://127.0.0.1:1/responses", 8192, 16, "saved")
                    .unwrap();
            let reply = reqwest::blocking::Client::new()
                .post(format!("{}/responses", gate.url))
                .bearer_auth("fixture")
                .json(&json!({"model":model,"input":[{"role":"user","content":task}]}))
                .send()
                .unwrap();
            assert_eq!(reply.status(), 502);
            assert!(gate.receipt().is_err());
        }
        let server = Server::new(vec![(200, responses("pinned", "ready"))]);
        let gate = Gate::start_bounded(
            "pinned",
            &format!("{}/responses", server.url),
            8192,
            4,
            "saved",
        )
        .unwrap();
        let reply = reqwest::blocking::Client::new()
            .post(format!("{}/responses", gate.url))
            .bearer_auth("fixture")
            .json(&json!({"model":"pinned","input":[{"role":"user","content":"saved"}]}))
            .send()
            .unwrap();
        assert_eq!(reply.status(), 502);
        assert!(reply.text().unwrap().is_empty());
        assert!(gate.receipt().is_err());
        assert_eq!(server.finish().len(), 1);
        assert!(
            Gate::start_bounded("pinned", "http://127.0.0.1:1/responses", 1, 4, "saved").is_err()
        );
    }
    #[test]
    fn team_gate_gemini_auxiliary_is_local_and_main_cap_is_reserved() {
        let api = "gemini-3.1-pro-preview";
        let server = Server::new(vec![(200, gemini_frame(&gemini_data()))]);
        let gate = Gate::start_bounded(
            api,
            &format!(
                "{}/v1beta/models/{api}:streamGenerateContent?alt=sse",
                server.url
            ),
            8192,
            16,
            "saved",
        )
        .unwrap();
        let client = reqwest::blocking::Client::new();
        let auxiliary=client.post(format!("{}/v1beta/models/title-model:streamGenerateContent?alt=sse",gate.url)).header("x-goog-api-key","fixture").json(&json!({"contents":[{"role":"user","parts":[{"text":"saved"}]}],"systemInstruction":{"parts":[{"text":"title"}]}})).send().unwrap();
        assert_eq!(auxiliary.status(), 400);
        let main=client.post(format!("{}/v1beta/models/{api}:streamGenerateContent?alt=sse",gate.url)).header("x-goog-api-key","fixture").json(&json!({"contents":[{"role":"user","parts":[{"text":"wrapper saved wrapper"}]}],"systemInstruction":{"parts":[{"text":super::super::team_completion::INSTRUCTIONS}]},"generationConfig":{"maxOutputTokens":65535,"thinkingConfig":{"thinkingBudget":8}},"tools":[{"functionDeclarations":[{"name":"native-shell"}]}]})).send().unwrap();
        assert_eq!(main.status(), 200);
        assert_eq!(gate.receipt().unwrap().usage.output_tokens, 7);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["generationConfig"]["maxOutputTokens"], 16);
        assert!(requests[0].body.get("tools").is_none());
        assert_eq!(requests[0].body["contents"][0]["parts"][0]["text"], "saved");
    }
    #[test]
    fn team_gate_bounded_chat_replaces_context_and_keeps_complete_usage() {
        let data=format!("data: {}\n\ndata: {}\n\ndata: [DONE]\n\n",json!({"model":"pinned","choices":[{"index":0,"delta":{"content":"ready"},"finish_reason":null}]}),json!({"model":"pinned","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":100,"completion_tokens":5}})).into_bytes();
        let server = Server::new(vec![(200, data)]);
        let gate = Gate::start_bounded(
            "pinned",
            &format!("{}/chat/completions", server.url),
            8192,
            16,
            "saved",
        )
        .unwrap();
        let reply=reqwest::blocking::Client::new().post(format!("{}/chat/completions",gate.url)).bearer_auth("fixture-only").json(&json!({"model":"pinned","max_tokens":99999,"messages":[{"role":"system","content":"native context"},{"role":"user","content":"saved"}],"tools":[{"type":"function","function":{"name":"shell"}}]})).send().unwrap();
        assert_eq!(reply.status(), 200);
        assert_eq!(gate.receipt().unwrap().usage.output_tokens, 5);
        let requests = server.finish();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].body["max_tokens"], 16);
        assert_eq!(requests[0].body["tools"], json!([]));
        assert_eq!(
            requests[0].body["messages"][0]["content"],
            super::super::team_completion::INSTRUCTIONS
        );
        assert_eq!(requests[0].body["stream_options"]["include_usage"], true);
    }
}
