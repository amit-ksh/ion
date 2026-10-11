import {
  portfolioId,
  portfolioUtf8Bytes,
  type InboxHistoryStatus,
  type InboxHistoryView,
  type InboxKind,
  type InboxQuery,
  type PortfolioQuery,
  type PortfolioResult,
  type ProviderKind,
  type UsageLedgerQuery,
  type UsageMoney,
} from "@waku/client";
import { Stack } from "expo-router";
import { useEffect, useMemo, useState } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import {
  AutomationChoice,
  AutomationField,
} from "@/components/automation-form-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import { routineTime } from "@/lib/automation-api";

type Inbox = Extract<PortfolioResult, { kind: "inbox" }>;
type Usage = Extract<PortfolioResult, { kind: "usage" }>;
type Row =
  | { kind: "inbox"; entry: Inbox["page"]["entries"][number] }
  | { kind: "usage"; entry: Usage["page"]["entries"][number] };
const option = (value: string, label: string) => ({ value, label });
const views = [
  ["inbox", "Inbox"],
  ["unread", "Unread"],
  ["acknowledged", "Acknowledged"],
  ["archived", "Archived"],
  ["all", "All notifications"],
].map(([v, label]) => option(v!, label!));
const inboxKinds = [
  ["", "All types"],
  ["proposal", "Proposed tasks"],
  ["decision", "Decisions"],
  ["approval", "Approvals"],
  ["question", "Questions"],
  ["issue", "Issues"],
  ["completed", "Completed work"],
  ["artifact", "Documents"],
  ["update", "Updates"],
].map(([v, label]) => option(v!, label!));
const statuses = [
  option("all", "All statuses"),
  option("needsAction", "Needs action"),
  option("resolved", "Resolved"),
];
const providers: [ProviderKind, string][] = [
  ["nebius", "Nebius"],
  ["claude", "Claude"],
  ["codex", "Codex"],
  ["openCode", "OpenCode"],
  ["openCode2", "OpenCode 2"],
  ["antigravity", "Antigravity"],
  ["deepSeek", "DeepSeek"],
  ["grok", "Grok"],
  ["kimi", "Kimi"],
  ["fx", "Fx"],
  ["pi", "Pi"],
  ["ohMyPi", "Oh My Pi"],
  ["amp", "Amp"],
  ["cursor", "Cursor"],
];
const emptyInbox = (): InboxQuery => ({
  projectId: null,
  taskId: null,
  kind: null,
  view: "inbox",
  status: "all",
  after: null,
  limit: 32,
});
const emptyUsage = (): UsageLedgerQuery => ({
  projectId: null,
  taskId: null,
  agentId: null,
  provider: null,
  environment: null,
  kind: null,
  model: null,
  since: null,
  after: null,
  limit: 32,
});
const money = (value: UsageMoney) =>
  `${value.currency} ${(value.micros / 1_000_000).toFixed(6)}`;

export default function GatewayPortfolioScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets();
  const [mode, setMode] = useState<"inbox" | "usage">("inbox");
  const [inbox, setInbox] = useState<InboxQuery>(emptyInbox),
    [usage, setUsage] = useState<UsageLedgerQuery>(emptyUsage);
  const [previous, setPrevious] = useState<(number | null)[]>([]);
  const [advanced, setAdvanced] = useState(false),
    [error, setError] = useState<string | null>(null);
  const [projectInput, setProjectInput] = useState(""),
    [taskInput, setTaskInput] = useState(""),
    [agentInput, setAgentInput] = useState(""),
    [modelInput, setModelInput] = useState("");
  const [hours, setHours] = useState("");
  const query = useMemo<Exclude<PortfolioQuery, { kind: "projects" }>>(
    () =>
      mode === "inbox"
        ? { kind: "inbox", query: inbox }
        : { kind: "usage", query: usage },
    [mode, inbox, usage],
  );
  const state = useGatewayPortfolio(query);
  const identity = state.pairing
    ? `${state.pairing.baseUrl}:${state.pairing.id}:${state.ownerId}`
    : "";
  useEffect(() => {
    setInbox(emptyInbox());
    setUsage(emptyUsage());
    setPrevious([]);
    setHours("");
    setError(null);
    setProjectInput("");
    setTaskInput("");
    setAgentInput("");
    setModelInput("");
  }, [identity]);
  const result = state.result,
    selected = query.query;
  const rows = useMemo<Row[]>(
    () =>
      result?.kind === "inbox"
        ? result.page.entries.map((entry) => ({ kind: "inbox", entry }))
        : result?.kind === "usage"
          ? result.page.entries.map((entry) => ({ kind: "usage", entry }))
          : [],
    [result],
  );
  const names = useMemo(
    () =>
      new Map(
        state.projects.projects.map((project) => [project.id, project.name]),
      ),
    [state.projects],
  );
  const projectOptions = useMemo(() => {
    const all = new Map(names);
    rows.forEach(({ entry }) => {
      if (entry.projectId && !all.has(entry.projectId))
        all.set(entry.projectId, `Project ${entry.projectId.slice(0, 8)}`);
    });
    if (selected.projectId && !all.has(selected.projectId))
      all.set(selected.projectId, `Project ${selected.projectId.slice(0, 8)}`);
    return [
      option("", "All projects"),
      ...[...all].map(([id, name]) => option(id, name)),
    ];
  }, [names, rows, selected.projectId]);
  const taskOptions = useMemo(() => {
    const ids = new Set(
      rows.map(({ entry }) => entry.taskId).filter((id): id is string => !!id),
    );
    if (selected.taskId) ids.add(selected.taskId);
    return [
      option("", "All tasks"),
      ...[...ids].map((id) => option(id, `Task ${id.slice(0, 8)}`)),
    ];
  }, [rows, selected.taskId]);
  function reset() {
    setPrevious([]);
    setError(null);
  }
  function setProject(value: string) {
    setProjectInput(value);
    setTaskInput("");
    reset();
    setInbox((q) => ({
      ...q,
      projectId: value || null,
      taskId: null,
      after: null,
    }));
    setUsage((q) => ({
      ...q,
      projectId: value || null,
      taskId: null,
      after: null,
    }));
  }
  function setTask(value: string) {
    setTaskInput(value);
    reset();
    setInbox((q) => ({ ...q, taskId: value || null, after: null }));
    setUsage((q) => ({ ...q, taskId: value || null, after: null }));
  }
  function apply() {
    const project = projectInput.trim().toLowerCase(),
      task = taskInput.trim().toLowerCase(),
      agent = agentInput.trim().toLowerCase(),
      model = modelInput.trim();
    if (
      [project, task, ...(mode === "usage" ? [agent] : [])].some(
        (id) => id && !portfolioId(id),
      )
    ) {
      setError(
        "Use a full project, task or agent ID, or leave the field empty for all records.",
      );
      return;
    }
    if (mode === "usage" && portfolioUtf8Bytes(model) > 512) {
      setError("Model name is too long. Use its exact provider name.");
      return;
    }
    reset();
    setInbox((q) => ({
      ...q,
      projectId: project || null,
      taskId: task || null,
      after: null,
    }));
    setUsage((q) => ({
      ...q,
      projectId: project || null,
      taskId: task || null,
      agentId: agent || null,
      model: model || null,
      after: null,
    }));
  }
  function page(after: number | null) {
    if (mode === "inbox") setInbox((q) => ({ ...q, after }));
    else setUsage((q) => ({ ...q, after }));
  }
  const next = result?.kind === "projects" ? null : result?.page.nextCursor;
  const disabled = !state.ownerId;
  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen
        options={{
          title: mode === "inbox" ? "Gateway Inbox" : "Gateway usage",
        }}
      />
      <FlatList
        data={rows}
        keyExtractor={(row) => `${row.kind}:${row.entry.sequence}`}
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={{ paddingBottom: insets.bottom + 24 }}
        refreshControl={
          <RefreshControl
            refreshing={state.loading}
            onRefresh={() => void state.refresh()}
            tintColor={theme.textSecondary}
          />
        }
        ListHeaderComponent={
          <View style={styles.header}>
            <AutomationRefresh
              refreshing={state.loading}
              onRefresh={() => void state.refresh()}
            />
            <AutomationChoice
              label="Owner"
              value={state.ownerId}
              options={state.owners.map((owner) =>
                option(owner.publisherId, owner.name),
              )}
              disabled={!state.owners.length}
              onChange={state.selectOwner}
            />
            <View style={styles.actions}>
              {(["inbox", "usage"] as const).map((value) => (
                <AutomationButton
                  key={value}
                  label={value === "inbox" ? "Inbox" : "Usage"}
                  disabled={mode === value}
                  onPress={() => {
                    setMode(value);
                    setPrevious([]);
                    setError(null);
                  }}
                />
              ))}
            </View>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              Owner history across projects. Review pending actions on the owner
              or in the gateway approval views.
            </Text>
            <AutomationChoice
              label="Project"
              value={selected.projectId ?? ""}
              options={projectOptions}
              disabled={disabled}
              onChange={setProject}
            />
            {state.projects.nextCursor && (
              <AutomationButton
                label={`Load more projects (${state.projects.projects.length} of ${state.projects.total})`}
                disabled={disabled || state.loading}
                onPress={() => void state.moreProjects()}
              />
            )}
            <AutomationChoice
              label="Task"
              value={selected.taskId ?? ""}
              options={taskOptions}
              disabled={disabled}
              onChange={setTask}
            />
            <AutomationButton
              label={advanced ? "Hide filters" : "More filters"}
              onPress={() => setAdvanced(!advanced)}
            />
            {advanced && (
              <View style={styles.filters}>
                {mode === "inbox" ? (
                  <>
                    <AutomationChoice
                      label="Notifications"
                      value={inbox.view}
                      options={views}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setInbox({
                          ...inbox,
                          view: v as InboxHistoryView,
                          after: null,
                        });
                      }}
                    />
                    <AutomationChoice
                      label="Type"
                      value={inbox.kind ?? ""}
                      options={inboxKinds}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setInbox({
                          ...inbox,
                          kind: (v || null) as InboxKind | null,
                          after: null,
                        });
                      }}
                    />
                    <AutomationChoice
                      label="Action status"
                      value={inbox.status}
                      options={statuses}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setInbox({
                          ...inbox,
                          status: v as InboxHistoryStatus,
                          after: null,
                        });
                      }}
                    />
                  </>
                ) : (
                  <>
                    <AutomationChoice
                      label="Harness"
                      value={usage.provider ?? ""}
                      options={[
                        option("", "All harnesses"),
                        ...providers.map(([v, label]) => option(v, label)),
                      ]}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setUsage({
                          ...usage,
                          provider: (v || null) as ProviderKind | null,
                          after: null,
                        });
                      }}
                    />
                    <AutomationChoice
                      label="Environment"
                      value={usage.environment ?? ""}
                      options={[
                        option("", "All environments"),
                        option("local", "Local"),
                        option("nebiusSandbox", "Nebius"),
                      ]}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setUsage({
                          ...usage,
                          environment: (v ||
                            null) as UsageLedgerQuery["environment"],
                          after: null,
                        });
                      }}
                    />
                    <AutomationChoice
                      label="Usage type"
                      value={usage.kind ?? ""}
                      options={[
                        option("", "All usage"),
                        option("modelCall", "Team model calls"),
                        option("soloModelCall", "Task model calls"),
                        option("remoteCommand", "Remote commands"),
                        option("gpuJob", "GPU jobs"),
                      ]}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setUsage({
                          ...usage,
                          kind: (v || null) as UsageLedgerQuery["kind"],
                          after: null,
                        });
                      }}
                    />
                    <AutomationChoice
                      label="Time range"
                      value={hours}
                      options={[
                        option("", "All time"),
                        option("24", "Last 24 hours"),
                        option("168", "Last 7 days"),
                        option("720", "Last 30 days"),
                      ]}
                      disabled={disabled}
                      onChange={(v) => {
                        reset();
                        setHours(v);
                        setUsage({
                          ...usage,
                          since: v
                            ? Math.max(
                                0,
                                Math.floor(Date.now() / 1000) -
                                  Number(v) * 3600,
                              )
                            : null,
                          after: null,
                        });
                      }}
                    />
                    <AutomationField
                      label="Agent ID"
                      value={agentInput}
                      onChangeText={setAgentInput}
                      placeholder="All agents"
                      autoCapitalize="none"
                      autoCorrect={false}
                    />
                    <AutomationField
                      label="Exact model name"
                      value={modelInput}
                      onChangeText={setModelInput}
                      placeholder="All models"
                      autoCapitalize="none"
                      autoCorrect={false}
                    />
                  </>
                )}
                <AutomationField
                  label="Project ID"
                  value={projectInput}
                  onChangeText={setProjectInput}
                  placeholder="All projects"
                  autoCapitalize="none"
                  autoCorrect={false}
                />
                <AutomationField
                  label="Task ID"
                  value={taskInput}
                  onChangeText={setTaskInput}
                  placeholder="All tasks"
                  autoCapitalize="none"
                  autoCorrect={false}
                />
                <AutomationButton
                  label={mode === "inbox" ? "Apply IDs" : "Apply IDs and model"}
                  disabled={disabled}
                  onPress={apply}
                />
              </View>
            )}
            {result && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Updated {new Date(result.servedAt).toLocaleString()}
              </Text>
            )}
            {result?.kind === "inbox" && (
              <>
                <Text style={[styles.body, { color: theme.text }]}>
                  {result.page.matchingCount} matching notifications ·{" "}
                  {result.page.needsActionCount} need action
                </Text>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {result.page.unreadCount} unread across projects
                </Text>
                {result.page.taskHistorySyncing && (
                  <Text style={[styles.body, { color: theme.textSecondary }]}>
                    Importing task history… Refresh for the latest records.
                  </Text>
                )}
              </>
            )}
            {result?.kind === "usage" && (
              <>
                <Text style={[styles.body, { color: theme.text }]}>
                  {result.page.summary.records} records ·{" "}
                  {result.page.summary.modelCalls} model requests
                </Text>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {result.page.summary.inputTokens} input /{" "}
                  {result.page.summary.outputTokens} output tokens ·{" "}
                  {result.page.summary.reservedTokens} reserved ·{" "}
                  {result.page.summary.unknownCalls} requests awaiting usage
                </Text>
                {result.page.summary.costs.map((cost) => (
                  <Text
                    key={`${cost.amount.currency}:${cost.quality}:${cost.finality}`}
                    style={[styles.body, { color: theme.textSecondary }]}
                  >
                    {money(cost.amount)} · {cost.quality} · {cost.finality}
                  </Text>
                ))}
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {result.page.summary.unknownCostRecords} costs unavailable
                </Text>
              </>
            )}
            {result?.pageTruncated && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                This transfer contains fewer rows. Next continues the same
                history; totals cover all matching records.
              </Text>
            )}
            {(error || state.message) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {error ?? state.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.loading
              ? "Loading owner history…"
              : result
                ? "No records match these filters."
                : "Refresh to load owner history."}
          </Text>
        }
        ListFooterComponent={
          <View style={[styles.actions, styles.footer]}>
            {!!previous.length && (
              <AutomationButton
                label="Previous"
                disabled={state.loading || !result}
                onPress={() => {
                  page(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {next != null && (
              <AutomationButton
                label="Next"
                disabled={state.loading}
                onPress={() => {
                  setPrevious([...previous, selected.after]);
                  page(next);
                }}
              />
            )}
          </View>
        }
        renderItem={({ item }) =>
          item.kind === "inbox" ? (
            <View style={[styles.row, { borderBottomColor: theme.border }]}>
              <Text style={[styles.title, { color: theme.text }]}>
                {item.entry.title}
              </Text>
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {names.get(item.entry.projectId ?? "") ??
                  (item.entry.projectId
                    ? `Project ${item.entry.projectId.slice(0, 8)}`
                    : "Workspace")}{" "}
                · {routineTime(item.entry.occurredAt)} ·{" "}
                {item.entry.state.archived
                  ? "Archived"
                  : item.entry.state.acknowledged
                    ? "Acknowledged"
                    : item.entry.state.read
                      ? "Read"
                      : "Unread"}
                {item.entry.needsAction ? " · Needs action" : ""}
              </Text>
              <Text selectable style={[styles.body, { color: theme.text }]}>
                {item.entry.detail}
              </Text>
              {!item.entry.detailComplete && (
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  Preview shortened. Open the source on the owner for the full
                  detail.
                </Text>
              )}
              {item.entry.taskId && (
                <Text
                  selectable
                  style={[styles.body, { color: theme.textSecondary }]}
                >
                  Task {item.entry.taskId}
                </Text>
              )}
            </View>
          ) : (
            <View style={[styles.row, { borderBottomColor: theme.border }]}>
              <Text style={[styles.title, { color: theme.text }]}>
                {item.entry.model ??
                  (item.entry.kind === "gpuJob"
                    ? "GPU job"
                    : item.entry.kind === "remoteCommand"
                      ? "Remote command"
                      : "Model request")}
              </Text>
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {names.get(item.entry.projectId) ??
                  `Project ${item.entry.projectId.slice(0, 8)}`}{" "}
                · {item.entry.agentName ?? "Task agent"} ·{" "}
                {item.entry.provider ?? "Unknown harness"} ·{" "}
                {item.entry.environment === "nebiusSandbox"
                  ? "Nebius"
                  : item.entry.environment === "local"
                    ? "Local"
                    : "Unknown environment"}{" "}
                · {item.entry.status} ·{" "}
                {routineTime(item.entry.startedAt ?? item.entry.observedAt)}
              </Text>
              <Text style={[styles.body, { color: theme.text }]}>
                {item.entry.inputTokens ?? "Unknown"} input /{" "}
                {item.entry.outputTokens ?? "Unknown"} output tokens ·{" "}
                {item.entry.reservedTokens} reserved
              </Text>
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {item.entry.cost
                  ? `${money(item.entry.cost.amount)} · ${item.entry.cost.quality} · ${item.entry.cost.finality}`
                  : "Cost unavailable"}
                {item.entry.spendingAllowance
                  ? ` · Approved allowance ${money(item.entry.spendingAllowance)}`
                  : ""}
              </Text>
              <Text
                selectable
                style={[styles.body, { color: theme.textSecondary }]}
              >
                Task {item.entry.taskId}
                {item.entry.agentId ? ` · Agent ${item.entry.agentId}` : ""}
              </Text>
            </View>
          )
        }
      />
    </View>
  );
}
const styles = StyleSheet.create({
  screen: { flex: 1 },
  header: { paddingHorizontal: 20, paddingTop: 8, paddingBottom: 20, gap: 12 },
  filters: { gap: 12 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  footer: { padding: 20 },
  row: {
    paddingHorizontal: 20,
    paddingVertical: 20,
    gap: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  title: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  body: { fontSize: 14, lineHeight: 21 },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
