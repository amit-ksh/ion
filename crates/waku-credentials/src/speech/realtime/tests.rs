use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;

fn configured() -> TranscriptState {
    TranscriptState {
        created: true,
        updated: true,
        update_requested: true,
        ..Default::default()
    }
}

#[test]
fn session_configuration_acknowledgment_cannot_arrive_early() {
    let mut state = TranscriptState::default();
    assert!(
        state
            .accept(r#"{"type":"session.updated"}"#, false)
            .is_err()
    );
    assert!(
        state
            .accept(
                r#"{"type":"conversation.item.input_audio_transcription.delta","delta":"early"}"#,
                false
            )
            .is_err()
    );
    state
        .accept(r#"{"type":"session.created"}"#, false)
        .unwrap();
    assert!(
        state
            .accept(r#"{"type":"session.created"}"#, false)
            .is_err()
    );
    assert!(
        state
            .accept(r#"{"type":"session.updated"}"#, false)
            .is_err()
    );
    state.update_requested = true;
    state
        .accept(r#"{"type":"session.updated"}"#, false)
        .unwrap();
    assert!(
        state
            .accept(r#"{"type":"session.updated"}"#, false)
            .is_err()
    );
}

#[test]
fn only_commit_acknowledgment_publishes_all_final_segments() {
    let mut state = configured();
    state
        .accept(
            r#"{"type":"conversation.item.input_audio_transcription.delta","delta":"tentative"}"#,
            false,
        )
        .unwrap();
    assert_eq!(state.preview(), "tentative");
    assert!(state.final_text().unwrap().is_none());
    state.accept(r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"First."}"#, false).unwrap();
    state
        .accept(
            r#"{"type":"conversation.item.input_audio_transcription.delta","delta":" tail"}"#,
            false,
        )
        .unwrap();
    state.accept(r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"Second."}"#, true).unwrap();
    assert!(state.final_text().unwrap().is_none());
    state
        .accept(r#"{"type":"input_audio_buffer.committed"}"#, true)
        .unwrap();
    assert_eq!(state.final_text().unwrap().unwrap(), "First. Second.");
}

#[test]
fn empty_final_after_endpoint_does_not_require_another_nonempty_segment() {
    let mut state = configured();
    state.accept(r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"Already ended."}"#, false).unwrap();
    state
        .accept(
            r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":""}"#,
            true,
        )
        .unwrap();
    state
        .accept(r#"{"type":"input_audio_buffer.committed"}"#, true)
        .unwrap();
    assert_eq!(state.final_text().unwrap().unwrap(), "Already ended.");
}

#[test]
fn rejects_unfinalized_partial_empty_malformed_and_oversized_events() {
    let mut state = configured();
    assert!(
        state
            .accept(r#"{"type":"input_audio_buffer.committed"}"#, false)
            .is_err()
    );
    assert!(
        state
            .accept(
                r#"{"type":"conversation.item.input_audio_transcription.delta","delta":42}"#,
                false
            )
            .is_err()
    );
    assert!(state.accept(&" ".repeat(MAX_EVENT + 1), false).is_err());
    assert!(state.accept(&serde_json::json!({"type":"conversation.item.input_audio_transcription.delta","delta":"x".repeat(MAX_TEXT)}).to_string(), false).is_err());
    state
        .accept(r#"{"type":"input_audio_buffer.committed"}"#, true)
        .unwrap();
    assert!(state.final_text().is_err());
    state.partial = "unfinalized".into();
    state.committed = "some text".into();
    assert!(state.final_text().is_err());
    let error = state
        .accept(
            r#"{"type":"error","error":{"message":"test-secret"}}"#,
            true,
        )
        .unwrap_err();
    assert!(!error.to_string().contains("test-secret"));
}

#[test]
fn authenticated_native_stream_configures_before_audio_and_waits_for_ack() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let service = SpeechService::new(
        format!("http://{}/", listener.local_addr().unwrap()),
        "test-secret".into(),
    )
    .unwrap();
    let worker = std::thread::spawn(move || {
        let (tcp, _) = listener.accept().unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut socket = tungstenite::accept_hdr(
            tcp,
            |request: &tungstenite::handshake::server::Request, response| {
                assert_eq!(request.uri().path(), "/v1/audio/transcriptions/realtime");
                assert!(request.uri().query().is_none());
                assert_eq!(request.headers()["Authorization"], "Bearer test-secret");
                Ok(response)
            },
        )
        .unwrap();
        socket
            .send(Message::Text(r#"{"type":"session.created"}"#.into()))
            .unwrap();
        let Message::Text(config) = socket.read().unwrap() else {
            panic!("configuration before PCM")
        };
        let config: Value = serde_json::from_str(&config).unwrap();
        assert_eq!(config["session"]["sample_rate"], 16000);
        socket
            .send(Message::Text(r#"{"type":"session.updated"}"#.into()))
            .unwrap();
        let Message::Binary(pcm) = socket.read().unwrap() else {
            panic!("expected PCM")
        };
        assert_eq!(pcm.as_ref(), &[1, 0, 255, 255]);
        socket
            .send(Message::Text(
                r#"{"type":"conversation.item.input_audio_transcription.delta","delta":"Preview"}"#
                    .into(),
            ))
            .unwrap();
        let Message::Text(commit) = socket.read().unwrap() else {
            panic!("expected commit")
        };
        assert_eq!(
            serde_json::from_str::<Value>(&commit).unwrap()["type"],
            "input_audio_buffer.commit"
        );
        socket.send(Message::Text(r#"{"type":"conversation.item.input_audio_transcription.completed","transcript":"Confirmed"}"#.into())).unwrap();
        socket
            .send(Message::Text(
                r#"{"type":"input_audio_buffer.committed"}"#.into(),
            ))
            .unwrap();
    });
    let mut session = service.start_live(16000, "auto").unwrap();
    session.send_pcm(&[1, -1]).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.last_preview.is_empty() && Instant::now() < deadline {
        session.poll().unwrap();
    }
    assert_eq!(session.last_preview, "Preview");
    assert!(session.final_transcript().unwrap().is_none());
    session.commit().unwrap();
    assert!(session.send_pcm(&[1]).is_err());
    while !session.state.acknowledged && Instant::now() < deadline {
        session.poll().unwrap();
    }
    assert_eq!(session.final_transcript().unwrap().unwrap(), "Confirmed");
    worker.join().unwrap();
}

#[test]
fn streaming_redirect_is_not_followed_and_handshake_error_hides_body() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let service = SpeechService::new(
        format!("http://{}/", listener.local_addr().unwrap()),
        "test-secret".into(),
    )
    .unwrap();
    let destination = TcpListener::bind("127.0.0.1:0").unwrap();
    destination.set_nonblocking(true).unwrap();
    let address = destination.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut tcp, _) = listener.accept().unwrap();
        let mut request = Vec::new();
        let mut byte = [0; 1];
        while !request.ends_with(b"\r\n\r\n") {
            tcp.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        write!(tcp,"HTTP/1.1 302 Found\r\nLocation: http://{address}/\r\nContent-Length: 11\r\nConnection: close\r\n\r\ntest-secret").unwrap();
    });
    let error = service.start_live(16000, "auto").err().unwrap();
    assert!(!error.to_string().contains("test-secret"));
    assert!(
        matches!(destination.accept(),Err(error) if error.kind()==std::io::ErrorKind::WouldBlock)
    );
    worker.join().unwrap();
}

#[test]
fn wss_selects_crypto_explicitly_and_does_not_send_bearer_before_tls() {
    assert!(matches!(
        tls_connector().unwrap(),
        tungstenite::Connector::Rustls(_)
    ));
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let service = SpeechService::new(
        format!("https://{}/", listener.local_addr().unwrap()),
        "test-secret".into(),
    )
    .unwrap();
    let worker = std::thread::spawn(move || {
        let (mut tcp, _) = listener.accept().unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut bytes = [0; 4096];
        let size = tcp.read(&mut bytes).unwrap();
        assert!(size > 0);
        assert!(
            !bytes[..size]
                .windows(b"test-secret".len())
                .any(|bytes| bytes == b"test-secret")
        );
        tcp.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .unwrap();
    });
    let error = service.start_live(16000, "auto").err().unwrap();
    assert!(error.to_string().contains("TLS"));
    worker.join().unwrap();
}

#[test]
fn explicit_connection_test_requires_stream_format_ack_without_audio() {
    for acknowledge in [true, false] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let service = SpeechService::new(
            format!("http://{}/", listener.local_addr().unwrap()),
            "fixture-private".into(),
        )
        .unwrap();
        let worker = std::thread::spawn(move || {
            let (mut http, _) = listener.accept().unwrap();
            http.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut request = Vec::new();
            let mut byte = [0];
            while !request.ends_with(b"\r\n\r\n") {
                http.read_exact(&mut byte).unwrap();
                request.push(byte[0]);
            }
            let request = String::from_utf8(request).unwrap();
            assert!(request.starts_with("GET /v1/models "));
            assert!(
                request
                    .to_ascii_lowercase()
                    .contains("authorization: bearer fixture-private")
            );
            let body = r#"{"data":[{"id":"nvidia/nemotron-3.5-asr-streaming-0.6b"}]}"#;
            write!(
                http,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
            drop(http);
            let (tcp, _) = listener.accept().unwrap();
            tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut socket = tungstenite::accept(tcp).unwrap();
            socket
                .send(Message::Text(r#"{"type":"session.created"}"#.into()))
                .unwrap();
            let Message::Text(config) = socket.read().unwrap() else {
                panic!("expected format only");
            };
            let config: Value = serde_json::from_str(&config).unwrap();
            assert_eq!(config["type"], "session.update");
            assert_eq!(config["session"]["sample_rate"], 16000);
            if acknowledge {
                socket
                    .send(Message::Text(r#"{"type":"session.updated"}"#.into()))
                    .unwrap();
                // The readiness check closes immediately; it must not send PCM or commit.
                assert!(socket.read().is_err());
            } else {
                socket
                    .send(Message::Text(
                        r#"{"type":"error","message":"fixture-private"}"#.into(),
                    ))
                    .unwrap();
            }
        });
        let result = service.test();
        assert_eq!(result.is_ok(), acknowledge);
        if let Err(error) = result {
            assert!(!error.to_string().contains("fixture-private"));
        }
        worker.join().unwrap();
    }
}
