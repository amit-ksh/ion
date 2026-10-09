use super::*;
use serde_json::json;
use waku_protocol::model::RuntimeMode;
struct Fixture {
    directory: std::path::PathBuf,
    scope: WorkspaceContext,
}
impl Fixture {
    fn new() -> Self {
        let directory = std::env::temp_dir().join(format!("ion-native-usage-{}", Uuid::new_v4()));
        Self {
            scope: WorkspaceContext {
                task_id: Uuid::new_v4(),
                project_id: Uuid::new_v4(),
                control_path: directory.join("control.db"),
                settings_path: directory.join("settings.json"),
                mode: RuntimeMode::Ask,
            },
            directory,
        }
    }
    fn reader(&self) -> NativeUsage {
        NativeUsage::new(&self.scope)
    }
    fn page(&self) -> waku_protocol::usage_ledger::UsageLedgerPage {
        ControlStore::new(self.scope.control_path.clone())
            .usage_ledger(crate::usage_ledger::all_query())
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}
fn claude(id: &str) -> Value {
    json!({"type":"assistant", "session_id":"session", "message":{"id":id,"model":"claude-model",
        "content":[{"type":"text","text":"private transcript"}],
        "usage":{"input_tokens":10,"cache_read_input_tokens":20,"cache_creation_input_tokens":30,"output_tokens":7}}})
}
fn codex(last_input: u64, total_input: u64, turn: &str) -> Value {
    json!({"method":"thread/tokenUsage/updated","params":{"threadId":"thread","turnId":turn,"tokenUsage":{
        "last":{"inputTokens":last_input,"cachedInputTokens":2,"outputTokens":5,"reasoningOutputTokens":3,"totalTokens":last_input+5},
        "total":{"inputTokens":total_input,"cachedInputTokens":2,"outputTokens":if total_input==last_input {5} else {10},
            "reasoningOutputTokens":3,"totalTokens":total_input+if total_input==last_input {5} else {10}},"modelContextWindow":200000}}})
}
fn start(turn: &str) -> Value {
    json!({"method":"turn/started","params":{"threadId":"thread","turn":{"id":turn}}})
}
#[test]
fn claude_block_replay_restart_and_fork_keep_one_receipt_and_original_owner() {
    let f = Fixture::new();
    let mut reader = f.reader();
    let value = claude("message-1");
    reader.observe_claude(&value, "session");
    reader.observe_claude(&value, "session");
    let first = f.page();
    assert_eq!(first.summary.model_calls, 1);
    assert_eq!(first.summary.input_tokens, 60);
    assert_eq!(first.summary.output_tokens, 7);
    assert_eq!(first.summary.unknown_cost_records, 1);
    assert_eq!(first.entries[0].model.as_deref(), Some("claude-model"));
    drop(reader);
    let mut scope = f.scope.clone();
    scope.task_id = Uuid::new_v4();
    scope.project_id = Uuid::new_v4();
    let mut resumed = NativeUsage::new(&scope);
    resumed.observe_claude(&value, "session");
    let replay = f.page();
    assert_eq!(replay.revision, first.revision);
    assert_eq!(replay.entries[0].task_id, f.scope.task_id);
    assert_eq!(replay.entries[0].project_id, f.scope.project_id);
    let mut copied = value.clone();
    copied["session_id"] = json!("fork-session");
    let mut fork = NativeUsage::new(&scope);
    fork.observe_claude(&copied, "fork-session");
    assert_eq!(f.page().revision, first.revision);
    resumed.observe_claude(&claude("message-2"), "session");
    assert_eq!(f.page().summary.model_calls, 2);
    let db = ConnectionForPrivacy::open(&f.scope.control_path);
    assert!(!db.contains("private transcript"));
    assert!(!db.contains("message-1"));
    assert!(!db.contains("session"));
}
// Read only the actual receipt rows, avoiding assumptions about SQLite page bytes.
struct ConnectionForPrivacy;
impl ConnectionForPrivacy {
    fn open(path: &std::path::Path) -> String {
        let db = rusqlite::Connection::open(path).unwrap();
        db.query_row(
            "SELECT group_concat(source_id||COALESCE(model,'')) FROM ion_usage_ledger",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }
}
#[test]
fn claude_invalid_and_partial_receipts_stay_unavailable_and_zero_is_known() {
    let f = Fixture::new();
    let mut reader = f.reader();
    reader.observe_claude(
        &json!({"type":"stream_event","message":claude("partial")["message"]}),
        "session",
    );
    reader.observe_claude(&json!({"type":"result","usage":{"input_tokens":999,"output_tokens":999},"total_cost_usd":42}),"session");
    for invalid in [
        json!(null),
        json!(-1),
        json!("7"),
        json!(1.25),
        json!(LIMIT + 1),
    ] {
        let mut value = claude("invalid");
        value["message"]["usage"]["output_tokens"] = invalid;
        reader.observe_claude(&value, "session");
    }
    let mut value = claude("cache-invalid");
    value["message"]["usage"]["cache_read_input_tokens"] = json!(null);
    reader.observe_claude(&value, "session");
    reader.observe_claude(&claude("other-session"), "different");
    assert_eq!(f.page().summary.records, 0);
    let mut value = claude("zero");
    value["message"]["usage"] = json!({"input_tokens":0,"output_tokens":0});
    reader.observe_claude(&value, "session");
    let page = f.page();
    assert_eq!(page.summary.model_calls, 1);
    assert_eq!(page.summary.unknown_calls, 0);
    assert_eq!(page.entries[0].input_tokens, Some(0));
}
#[test]
fn codex_resume_rate_limit_duplicates_context_fill_and_foreign_turns_are_not_calls() {
    let f = Fixture::new();
    let mut reader = f.reader();
    reader.observe_codex(&codex(10, 10, "old"), Some("thread")); // native historical replay
    reader.observe_codex(&start("turn"), Some("different-thread"));
    reader.observe_codex(&codex(10, 10, "turn"), Some("thread"));
    assert_eq!(f.page().summary.records, 0);
    reader.observe_codex(&start("turn"), Some("thread"));
    let first = codex(10, 10, "turn");
    reader.observe_codex(&first, Some("thread"));
    let initial = f.page();
    reader.observe_codex(&first, Some("thread"));
    let mut context = first.clone();
    context["params"]["tokenUsage"]["modelContextWindow"] = json!(1000000);
    reader.observe_codex(&context, Some("thread"));
    let mut synthetic = first.clone();
    for name in ["last", "total"] {
        synthetic["params"]["tokenUsage"][name] = json!({"inputTokens":0,"outputTokens":0,"cachedInputTokens":0,"reasoningOutputTokens":0,"totalTokens":200000});
    }
    reader.observe_codex(&synthetic, Some("thread"));
    assert_eq!(f.page().revision, initial.revision);
    reader.observe_codex(&codex(20, 30, "turn"), Some("thread")); // new actual call
    let page = f.page();
    assert_eq!(page.summary.model_calls, 2);
    assert_eq!(page.summary.input_tokens, 30);
    assert_eq!(page.summary.output_tokens, 10);
    assert!(page.entries.iter().all(|r| r.model.is_none()));
    drop(reader);
    let mut resumed = f.reader();
    resumed.observe_codex(&first, Some("thread"));
    resumed.observe_codex(&start("turn"), Some("thread"));
    resumed.observe_codex(&codex(20, 30, "turn"), Some("thread"));
    assert_eq!(f.page().revision, page.revision);
}
#[test]
fn native_owner_rejects_changed_receipt_and_concurrent_replay_is_atomic() {
    let f = Fixture::new();
    let id = identity(&["claude-message-v1", "message"]);
    let receipt = || NativeSoloReceipt {
        source_id: id,
        project_id: f.scope.project_id,
        task_id: f.scope.task_id,
        provider: ProviderKind::Claude,
        model: Some("claude-model".into()),
        usage: ModelTokenUsage {
            input_tokens: 5,
            output_tokens: 6,
        },
    };
    let store = ControlStore::new(f.scope.control_path.clone());
    store.record_native_solo_usage(&receipt(), 1).unwrap();
    let initial = f.page();
    std::thread::scope(|threads| {
        for _ in 0..4 {
            let r = receipt();
            let path = f.scope.control_path.clone();
            threads.spawn(move || {
                ControlStore::new(path)
                    .record_native_solo_usage(&r, 99)
                    .unwrap()
            });
        }
    });
    assert_eq!(f.page().revision, initial.revision);
    let mut changed = receipt();
    changed.usage.output_tokens = 7;
    assert!(store.record_native_solo_usage(&changed, 100).is_err());
    changed = receipt();
    changed.model = Some("edited-model".into());
    assert!(store.record_native_solo_usage(&changed, 100).is_err());
    assert_eq!(f.page().revision, initial.revision);
    assert_eq!(f.page().entries[0].started_at, Some(1));
}
#[test]
fn codex_invalid_subtotals_and_unbounded_ids_cannot_publish_receipts() {
    let f = Fixture::new();
    let mut reader = f.reader();
    reader.observe_codex(&start("turn"), Some("thread"));
    for field in [
        "inputTokens",
        "outputTokens",
        "cachedInputTokens",
        "reasoningOutputTokens",
        "totalTokens",
    ] {
        let mut value = codex(10, 10, "turn");
        value["params"]["tokenUsage"]["last"][field] = json!(null);
        reader.observe_codex(&value, Some("thread"));
    }
    let mut value = codex(10, 10, "turn");
    value["params"]["tokenUsage"]["last"]["cachedInputTokens"] = json!(11);
    reader.observe_codex(&value, Some("thread"));
    reader.observe_codex(&codex(10, 10, "unobserved"), Some("thread"));
    reader.observe_claude(&claude(&"x".repeat(257)), "session");
    assert_eq!(f.page().summary.records, 0);
    assert_ne!(identity(&["ab", "c"]), identity(&["a", "bc"]));
    assert_ne!(
        identity(&["claude-message-v1", "thread", "turn"]),
        identity(&["codex-call-v1", "thread", "turn"])
    );
}
