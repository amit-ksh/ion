//! Loopback-only HTTP fixtures: no account, credential or provider network access.
use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};
#[derive(Clone, Debug)]
pub(super) struct Request {
    pub line: String,
    pub headers: String,
    pub body: Value,
    pub raw_body: Vec<u8>,
}
pub(super) struct Server {
    pub url: String,
    pub requests: Arc<Mutex<Vec<Request>>>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Server {
    pub fn new(responses: Vec<(u16, Vec<u8>)>) -> Self {
        Self::with_hook(responses, |_| {})
    }
    pub fn with_timeout(responses: Vec<(u16, Vec<u8>)>, timeout: Duration) -> Self {
        Self::build(responses, |_, body| body, timeout, None, None)
    }
    pub fn with_timeout_and_optional_preflight(
        responses: Vec<(u16, Vec<u8>)>,
        timeout: Duration,
        preflight: &'static str,
    ) -> Self {
        Self::build(responses, |_, body| body, timeout, Some(preflight), None)
    }
    pub fn with_hook(
        responses: Vec<(u16, Vec<u8>)>,
        hook: impl Fn(&Request) + Send + 'static,
    ) -> Self {
        Self::build(
            responses,
            move |request, body| {
                hook(request);
                body
            },
            Duration::from_secs(20),
            None,
            None,
        )
    }
    pub fn with_response_handler(
        responses: Vec<(u16, Vec<u8>)>,
        hook: impl Fn(&Request, Vec<u8>) -> Vec<u8> + Send + 'static,
    ) -> Self {
        Self::build(responses, hook, Duration::from_secs(20), None, None)
    }
    pub fn with_optional_preflight(
        responses: Vec<(u16, Vec<u8>)>,
        hook: impl Fn(&Request, Vec<u8>) -> Vec<u8> + Send + 'static,
        preflight: &'static str,
    ) -> Self {
        Self::build(
            responses,
            hook,
            Duration::from_secs(20),
            Some(preflight),
            None,
        )
    }
    /// Native title requests can arrive independently of the main tool loop.
    /// Handle only recognized auxiliary requests without consuming its finite
    /// sequence of main responses.
    pub fn with_auxiliary(
        responses: Vec<(u16, Vec<u8>)>,
        auxiliary: impl Fn(&Request) -> Option<Vec<u8>> + Send + 'static,
    ) -> Self {
        Self::build(
            responses,
            |_, body| body,
            Duration::from_secs(65),
            None,
            Some(Box::new(auxiliary)),
        )
    }
    fn build(
        responses: Vec<(u16, Vec<u8>)>,
        hook: impl Fn(&Request, Vec<u8>) -> Vec<u8> + Send + 'static,
        timeout: Duration,
        preflight: Option<&'static str>,
        auxiliary: Option<Box<dyn Fn(&Request) -> Option<Vec<u8>> + Send>>,
    ) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let captured = requests.clone();
        let worker = thread::spawn(move || {
            let mut preflight_count = 0;
            let mut auxiliary_count = 0;
            for (status, body) in responses {
                let deadline = Instant::now() + timeout;
                loop {
                    let mut stream = loop {
                        match listener.accept() {
                            Ok((s, _)) => break s,
                            Err(e)
                                if e.kind() == std::io::ErrorKind::WouldBlock
                                    && Instant::now() < deadline =>
                            {
                                thread::sleep(Duration::from_millis(10))
                            }
                            Err(e) => panic!("Fixture request did not arrive: {e}"),
                        }
                    };
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(5)))
                        .unwrap();
                    let mut data = Vec::new();
                    let mut buf = [0; 4096];
                    let offset = loop {
                        let n = stream.read(&mut buf).unwrap();
                        assert!(n > 0);
                        data.extend_from_slice(&buf[..n]);
                        if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                            break i + 4;
                        }
                        assert!(data.len() < 16 * 1024);
                    };
                    let headers = String::from_utf8(data[..offset].to_vec()).unwrap();
                    let len = headers
                        .lines()
                        .filter_map(|l| l.split_once(':'))
                        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                        .map(|(_, v)| v.trim().parse::<usize>().unwrap())
                        .unwrap_or(0);
                    let binary = headers.lines().any(|h| {
                        h.to_ascii_lowercase() == "content-type: application/octet-stream"
                    });
                    assert!(
                        len <= if binary {
                            crate::teams::project::MAX_ARCHIVE
                        } else {
                            256 * 1024
                        }
                    );
                    while data.len() < offset + len {
                        let n = stream.read(&mut buf).unwrap();
                        assert!(n > 0);
                        data.extend_from_slice(&buf[..n]);
                    }
                    let request = Request {
                        line: headers.lines().next().unwrap().into(),
                        headers,
                        raw_body: data[offset..offset + len].to_vec(),
                        body: if len == 0 || binary {
                            Value::Null
                        } else {
                            serde_json::from_slice(&data[offset..offset + len]).unwrap()
                        },
                    };
                    if preflight == Some(request.line.as_str()) {
                        preflight_count += 1;
                        assert!(preflight_count <= 1, "Unexpected repeated native preflight");
                        assert_eq!(request.body, Value::Null);
                        captured.lock().unwrap().push(request);
                        stream.write_all(b"HTTP/1.1 200 Fixture\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                        continue;
                    }
                    if let Some(body) = auxiliary.as_ref().and_then(|handler| handler(&request)) {
                        auxiliary_count += 1;
                        assert!(
                            auxiliary_count <= 8,
                            "Unexpected repeated native auxiliary request"
                        );
                        captured.lock().unwrap().push(request);
                        write!(stream,"HTTP/1.1 200 Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                        stream.write_all(&body).unwrap();
                        continue;
                    }
                    let body = hook(&request, body);
                    captured.lock().unwrap().push(request);
                    write!(stream,"HTTP/1.1 {status} Fixture\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len()).unwrap();
                    stream.write_all(&body).unwrap();
                    break;
                }
            }
        });
        Self {
            url,
            requests,
            worker: Some(worker),
        }
    }
    pub fn finish(mut self) -> Vec<Request> {
        self.worker.take().unwrap().join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}
pub(super) fn responses(model: &str, text: &str) -> Vec<u8> {
    use serde_json::json;
    let item = json!({"id":"msg_fixture","type":"message","status":"completed","role":"assistant","phase":"final_answer","content":[{"type":"output_text","text":text,"annotations":[]}]});
    let response = json!({"id":"resp_fixture","object":"response","created_at":1,"status":"completed","model":model,"output":[item.clone()],"usage":{"input_tokens":100,"output_tokens":5,"total_tokens":105,"input_tokens_details":{"cached_tokens":0},"output_tokens_details":{"reasoning_tokens":0}}});
    let events = vec![
        json!({"type":"response.created","response":{"id":"resp_fixture","status":"in_progress"}}),
        json!({"type":"response.output_item.added","output_index":0,"item":{"id":"msg_fixture","type":"message","role":"assistant","content":[]}}),
        json!({"type":"response.content_part.added","item_id":"msg_fixture","output_index":0,"content_index":0,"part":{"type":"output_text","text":"","annotations":[]}}),
        json!({"type":"response.output_text.delta","item_id":"msg_fixture","output_index":0,"content_index":0,"delta":text}),
        json!({"type":"response.output_text.done","item_id":"msg_fixture","output_index":0,"content_index":0,"text":text}),
        json!({"type":"response.output_item.done","output_index":0,"item":item}),
        json!({"type":"response.completed","response":response}),
    ];
    events
        .iter()
        .map(|v| format!("event: {}\ndata: {v}\n\n", v["type"].as_str().unwrap()))
        .collect::<String>()
        .into_bytes()
}

pub(super) fn chat(model: &str, text: &str) -> Vec<u8> {
    use serde_json::json;
    let events = [
        json!({"id":"chat_fixture","object":"chat.completion.chunk","model":model,"choices":[{"index":0,"delta":{"role":"assistant","content":text},"finish_reason":null}]}),
        json!({"id":"chat_fixture","object":"chat.completion.chunk","model":model,"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}),
        json!({"id":"chat_fixture","object":"chat.completion.chunk","model":model,"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":5,"total_tokens":105}}),
    ];
    let mut out = events
        .iter()
        .map(|v| format!("data: {v}\n\n"))
        .collect::<String>();
    out.push_str("data: [DONE]\n\n");
    out.into_bytes()
}
