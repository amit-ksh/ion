import type {
  ExecutionEnvironment,
  ProviderKind,
  UsageCost,
  UsageLedgerKind,
  UsageMoney,
} from "@waku/client";
import { router } from "expo-router";
import { useEffect, useState } from "react";
import {
  FlatList,
  Pressable,
  RefreshControl,
  StyleSheet,
  Text,
  TextInput,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { ConnectionBanner } from "@/components/connection-banner";
import { useScreenHeaderInset } from "@/components/screen-header";
import { useAutomationQuery } from "@/hooks/use-automation-query";
import { useTaskState } from "@/hooks/use-daemon-data";
import { useTheme } from "@/hooks/use-theme";
import { routineTime } from "@/lib/automation-api";
const HARNESSES: [ProviderKind | null, string][] = [
  [null, "All harnesses"],
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
const KINDS: [UsageLedgerKind | null, string][] = [
  [null, "All usage"],
  ["modelCall", "Team model calls"],
  ["soloModelCall", "Task model calls"],
  ["remoteCommand", "Remote commands"],
  ["gpuJob", "GPU jobs"],
];
const ENVIRONMENTS: [ExecutionEnvironment | null, string][] = [
  [null, "All environments"],
  ["local", "Local"],
  ["nebiusSandbox", "Nebius"],
];
function money(amount: UsageMoney) {
  return `${amount.currency} ${(amount.micros / 1_000_000).toFixed(6)}`;
}
function costLabel(cost: UsageCost) {
  return `${money(cost.amount)} · ${cost.quality === "estimated" ? "Estimated" : "Reported"} · ${cost.finality === "final" ? "Final" : "Provisional"}`;
}
// Do not require TextEncoder to be installed in the native JS runtime.
function utf8Bytes(value: string): number {
  let bytes = 0;
  for (const character of value) {
    const point = character.codePointAt(0)!;
    bytes += point <= 0x7f ? 1 : point <= 0x7ff ? 2 : point <= 0xffff ? 3 : 4;
  }
  return bytes;
}
export default function UsageScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const header = useScreenHeaderInset();
  const tasks = useTaskState();
  const [project, setProject] = useState<string | null>(null);
  const [task, setTask] = useState<string | null>(null);
  const [agent, setAgent] = useState<string | null>(null);
  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [environment, setEnvironment] = useState<ExecutionEnvironment | null>(
    null,
  );
  const [kind, setKind] = useState<UsageLedgerKind | null>(null);
  const [modelInput, setModelInput] = useState("");
  const [model, setModel] = useState<string | null>(null);
  const [since, setSince] = useState<number | null>(null);
  const [hours, setHours] = useState<string | null>(null);
  const [after, setAfter] = useState<number | null>(null);
  const [previous, setPrevious] = useState<(number | null)[]>([]);
  const [error, setError] = useState<string | null>(null);
  const state = useAutomationQuery(
    [
      "usage",
      project,
      task,
      agent,
      provider,
      environment,
      kind,
      model,
      since,
      after,
    ],
    async (client) => {
      const response = await client.request({
        type: "loadUsageLedger",
        query: {
          projectId: project,
          taskId: task,
          agentId: agent,
          provider,
          environment,
          kind,
          model,
          since,
          after,
          limit: 32,
        },
      });
      if (response.type !== "usageLedger")
        throw new Error("Unexpected usage response. Refresh to retry.");
      return response.page;
    },
  );
  const { daemon, data: page, query, fresh } = state;
  const scope = `${daemon.activeProfile?.id}:${daemon.activeProfile?.address}:${daemon.connections}`;
  useEffect(() => {
    setProject(null);
    setTask(null);
    setAgent(null);
    setProvider(null);
    setEnvironment(null);
    setKind(null);
    setModelInput("");
    setModel(null);
    setSince(null);
    setHours(null);
    setAfter(null);
    setPrevious([]);
    setError(null);
  }, [scope]);
  function reset() {
    setAfter(null);
    setPrevious([]);
    setError(null);
  }
  function choices<T extends string | null>(
    values: readonly [T, string][],
    selected: T,
    change: (v: T) => void,
    label: string,
  ) {
    return (
      <FlatList
        horizontal
        accessibilityLabel={label}
        data={values}
        keyExtractor={([v]) => v ?? "all"}
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={styles.filters}
        renderItem={({ item: [v, name] }) => (
          <Pressable
            accessibilityRole="button"
            accessibilityState={{ selected: selected === v }}
            onPress={() => {
              change(v);
              reset();
            }}
            style={[
              styles.filter,
              {
                backgroundColor:
                  selected === v ? theme.overlayStrong : "transparent",
              },
            ]}
          >
            <Text style={[styles.body, { color: theme.text }]}>{name}</Text>
          </Pressable>
        )}
      />
    );
  }
  const agents = new Map(
    page?.entries
      .filter((e) => e.agentId)
      .map((e) => [e.agentId!, e.agentName ?? "Agent"]) ?? [],
  );
  if (agent && !agents.has(agent)) agents.set(agent, "Selected agent");
  const projects = new Map(
    tasks.data?.projects.map((p) => [p.id, p.name]) ?? [],
  );
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <FlatList
        data={page?.entries ?? []}
        keyExtractor={(e) => String(e.sequence)}
        contentContainerStyle={{
          paddingTop: header + 8,
          paddingBottom: insets.bottom + 24,
        }}
        refreshControl={
          <RefreshControl
            refreshing={query.isFetching}
            onRefresh={() => void query.refetch()}
            tintColor={theme.textSecondary}
          />
        }
        ListHeaderComponent={
          <View style={styles.header}>
            <ConnectionBanner />
            <AutomationRefresh
              refreshing={query.isFetching}
              onRefresh={() => void query.refetch()}
            />
            {choices(
              [
                [null, "All projects"],
                ...(tasks.data?.projects ?? []).map(
                  (p) => [p.id, p.name] as [string, string],
                ),
              ],
              project,
              (v) => {
                setProject(v);
                setTask(null);
              },
              "Project filter",
            )}
            {choices(
              [
                [null, "All tasks"],
                ...(tasks.data?.sessions ?? [])
                  .filter((s) => !project || s.project_id === project)
                  .map(
                    (s) =>
                      [s.id, s.auto_title || s.title || "Task"] as [
                        string,
                        string,
                      ],
                  ),
              ],
              task,
              setTask,
              "Task filter",
            )}
            {choices(
              [[null, "All agents"], ...agents.entries()],
              agent,
              setAgent,
              "Agent filter",
            )}
            {choices(HARNESSES, provider, setProvider, "Harness filter")}
            {choices(
              ENVIRONMENTS,
              environment,
              setEnvironment,
              "Execution environment filter",
            )}
            {choices(KINDS, kind, setKind, "Usage type")}
            {choices(
              [
                [null, "All time"],
                ["24", "Last 24 hours"],
                ["168", "Last 7 days"],
                ["720", "Last 30 days"],
              ],
              hours,
              (v) => {
                setHours(v);
                setSince(
                  v
                    ? Math.max(
                        0,
                        Math.floor(Date.now() / 1000) - Number(v) * 3600,
                      )
                    : null,
                );
              },
              "Time range",
            )}
            <View style={styles.actions}>
              <TextInput
                accessibilityLabel="Exact model name"
                placeholder="All models"
                placeholderTextColor={theme.textSecondary}
                value={modelInput}
                onChangeText={setModelInput}
                autoCapitalize="none"
                autoCorrect={false}
                style={[
                  styles.input,
                  { color: theme.text, borderColor: theme.border },
                ]}
              />
              <AutomationButton
                label="Apply model"
                onPress={() => {
                  if (utf8Bytes(modelInput.trim()) > 512) {
                    setError("Model name exceeds 512 bytes");
                    return;
                  }
                  setModel(modelInput.trim() || null);
                  reset();
                }}
              />
            </View>
            {page && (
              <>
                <Text style={[styles.body, { color: theme.text }]}>
                  {page.summary.records} records · {page.summary.modelCalls}{" "}
                  model requests
                </Text>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {page.summary.inputTokens} input / {page.summary.outputTokens}{" "}
                  output tokens · {page.summary.reservedTokens} reserved ·{" "}
                  {page.summary.unknownCalls} requests awaiting usage
                </Text>
                {page.summary.costs.map((c) => (
                  <Text
                    key={`${c.amount.currency}:${c.quality}:${c.finality}`}
                    style={[styles.body, { color: theme.textSecondary }]}
                  >
                    {money(c.amount)} {c.quality} ({c.finality})
                  </Text>
                ))}
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {page.summary.unknownCostRecords} costs unavailable
                </Text>
              </>
            )}
            {!fresh && page && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Saved usage · refresh to update
              </Text>
            )}
            {(error || query.error) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {error ?? query.error?.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {query.isFetching ? "Loading…" : "No usage matches these filters."}
          </Text>
        }
        ListFooterComponent={
          <View style={[styles.actions, { paddingHorizontal: 20 }]}>
            {!!previous.length && (
              <AutomationButton
                label="Previous"
                disabled={query.isFetching || !fresh}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {page?.nextCursor != null && (
              <AutomationButton
                label="Next"
                disabled={query.isFetching || !fresh}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(page.nextCursor);
                }}
              />
            )}
          </View>
        }
        renderItem={({ item: e }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.title, { color: theme.text }]}>
              {e.model ??
                (e.kind === "gpuJob"
                  ? "GPU job"
                  : e.kind === "remoteCommand"
                    ? "Remote command"
                    : "Model request")}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {projects.get(e.projectId) ??
                `Project ${e.projectId.slice(0, 8)}`}{" "}
              · {e.agentName ?? "Task agent"} ·{" "}
              {e.provider ?? "Unknown harness"} · {e.host} · {e.status} ·{" "}
              {routineTime(e.startedAt ?? e.observedAt)}
            </Text>
            <Text style={[styles.body, { color: theme.text }]}>
              {e.inputTokens != null && e.outputTokens != null
                ? `${e.inputTokens} input · ${e.outputTokens} output tokens`
                : e.kind === "modelCall" || e.kind === "soloModelCall"
                  ? `Token usage unavailable · ${e.reservedTokens} tokens reserved`
                  : "Resource usage"}
              {e.elapsedMilliseconds != null
                ? ` · ${(e.elapsedMilliseconds / 1000).toFixed(3)}s elapsed`
                : ""}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {e.cost ? costLabel(e.cost) : "Cost unavailable"}
              {e.spendingAllowance
                ? ` · Approved allowance ${money(e.spendingAllowance)}`
                : ""}
            </Text>
            <View style={styles.actions}>
              <AutomationButton
                label="Open task"
                disabled={!fresh}
                onPress={() =>
                  router.push({
                    pathname: "/session/[id]",
                    params: { id: e.taskId },
                  })
                }
              />
              {e.agentId && (
                <AutomationButton
                  label="Filter agent"
                  onPress={() => {
                    setAgent(e.agentId);
                    reset();
                  }}
                />
              )}
              {e.model && (
                <AutomationButton
                  label="Filter model"
                  onPress={() => {
                    setModelInput(e.model!);
                    setModel(e.model);
                    reset();
                  }}
                />
              )}
            </View>
          </View>
        )}
      />
    </View>
  );
}
const styles = StyleSheet.create({
  header: { paddingHorizontal: 20, gap: 12, paddingBottom: 20 },
  filters: { gap: 6 },
  filter: {
    minHeight: 44,
    justifyContent: "center",
    paddingHorizontal: 12,
    paddingVertical: 10,
    borderRadius: 8,
  },
  body: { fontSize: 14, lineHeight: 21 },
  title: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  row: {
    paddingHorizontal: 20,
    paddingVertical: 20,
    gap: 10,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  input: {
    minHeight: 44,
    padding: 10,
    borderRadius: 8,
    borderWidth: StyleSheet.hairlineWidth,
    minWidth: 200,
    flex: 1,
  },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
