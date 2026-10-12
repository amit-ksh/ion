import { Stack, router } from "expo-router";
import { useCallback, useEffect, useState } from "react";
import {
  FlatList,
  Pressable,
  RefreshControl,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import type {
  WakuClient,
  RemoteExecutionCursor,
  RemoteExecutionKind,
} from "@waku/client";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { useTheme } from "@/hooks/use-theme";
import { useAutomationQuery } from "@/hooks/use-automation-query";
import { useTaskState } from "@/hooks/use-daemon-data";
import { readRemoteExecutions } from "@/lib/remote-executions";
const KINDS: [RemoteExecutionKind | null, string][] = [
  [null, "All types"],
  ["model", "Model calls"],
  ["command", "Commands"],
  ["gpu", "GPU jobs"],
];
export default function RemoteExecutionsScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    tasks = useTaskState();
  const [project, setProject] = useState<string | null>(null),
    [kind, setKind] = useState<RemoteExecutionKind | null>(null),
    [unresolved, setUnresolved] = useState(true);
  const [after, setAfter] = useState<RemoteExecutionCursor | null>(null),
    [previous, setPrevious] = useState<(RemoteExecutionCursor | null)[]>([]);
  const load = useCallback(
    (client: WakuClient) =>
      readRemoteExecutions(client, {
        projectId: project,
        kind,
        unresolvedOnly: unresolved,
        after,
        limit: 32,
      }),
    [project, kind, unresolved, after],
  );
  const state = useAutomationQuery(
    ["remote-executions", project, kind, unresolved, after],
    load,
    { poll: false },
  );
  useEffect(() => {
    setAfter(null);
    setPrevious([]);
    setProject(null);
  }, [state.daemon.activeProfile?.id, state.daemon.activeProfile?.address]);
  const data = state.fresh && !state.query.isFetching ? state.data : undefined;
  const reset = () => {
    setAfter(null);
    setPrevious([]);
  };
  const choices = <T extends string | null>(
    items: [T, string][],
    selected: T,
    choose: (value: T) => void,
  ) => (
    <View style={styles.filters}>
      {items.map(([value, label]) => (
        <Pressable
          key={value ?? "all"}
          accessibilityRole="button"
          accessibilityState={{ selected: value === selected }}
          onPress={() => {
            choose(value);
            reset();
          }}
          style={[
            styles.filter,
            {
              backgroundColor:
                value === selected ? theme.surface : theme.background,
              borderColor: theme.border,
            },
          ]}
        >
          <Text style={{ color: theme.text }}>{label}</Text>
        </Pressable>
      ))}
    </View>
  );
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <Stack.Screen options={{ title: "Remote executions" }} />
      <FlatList
        data={data?.entries ?? []}
        keyExtractor={(row) => `${row.kind}:${row.requestId}`}
        contentContainerStyle={{ paddingBottom: insets.bottom + 24 }}
        refreshControl={
          <RefreshControl
            refreshing={state.query.isFetching}
            onRefresh={() => void state.query.refetch()}
            tintColor={theme.text}
          />
        }
        ListHeaderComponent={
          <View style={styles.header}>
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              Remote executions
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              Saved sandbox and GPU records. Open the team task to manage work.
              Terminal receipts do not settle billing.
            </Text>
            <AutomationRefresh
              refreshing={state.query.isFetching}
              onRefresh={() => void state.query.refetch()}
            />
            {choices(
              [
                [null, "All projects"],
                ...(tasks.data?.projects ?? []).map(
                  (p) => [p.id, p.name] as [string, string],
                ),
              ],
              project,
              setProject,
            )}
            {choices(KINDS, kind, setKind)}
            <AutomationButton
              label={unresolved ? "Unresolved" : "All records"}
              onPress={() => {
                setUnresolved(!unresolved);
                reset();
              }}
            />
            {state.query.isError && (
              <Text style={[styles.body, { color: theme.text }]}>
                {state.query.error.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          !state.query.isFetching && data ? (
            <Text style={[styles.empty, { color: theme.textSecondary }]}>
              No remote executions match these filters.
            </Text>
          ) : !state.daemon.client ? (
            <Text style={[styles.empty, { color: theme.textSecondary }]}>
              Connect to the owner daemon to view remote executions.
            </Text>
          ) : null
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.heading, { color: theme.text }]}>
              {item.kind === "gpu"
                ? "GPU job"
                : item.kind === "command"
                  ? "Sandbox command"
                  : "Sandbox model call"}{" "}
              · {item.objective}
            </Text>
            <Text style={[styles.body, { color: theme.text }]}>
              {item.teamName} · {item.state} ·{" "}
              {item.settled
                ? "Terminal receipt recorded"
                : item.stopRequested
                  ? "Stop requested · awaiting terminal receipt"
                  : "Unresolved"}
            </Text>
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              {item.remoteProject} · Request {item.requestId}
              {item.operationId
                ? ` · Operation ${item.operationId}`
                : !item.settled
                  ? " · Operation handle unavailable"
                  : ""}
              {item.jobId ? ` · Job ${item.jobId}` : ""}
              {item.cleanupState ? ` · Cleanup ${item.cleanupState}` : ""}
            </Text>
            <AutomationButton
              label="Open team task"
              onPress={() =>
                router.push({
                  pathname: "/session/[id]",
                  params: { id: item.rootTaskId },
                })
              }
            />
          </View>
        )}
        ListFooterComponent={
          <View style={styles.filters}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous"
                disabled={!data}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {data?.nextCursor && (
              <AutomationButton
                label="Next"
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(data.nextCursor);
                }}
              />
            )}
          </View>
        }
      />
    </View>
  );
}
const styles = StyleSheet.create({
  header: { padding: 20, gap: 12 },
  title: { fontSize: 22, fontWeight: "600" },
  body: { fontSize: 14, lineHeight: 21 },
  heading: { fontSize: 16, fontWeight: "600" },
  row: { padding: 20, gap: 10, borderBottomWidth: 1 },
  filters: { padding: 8, flexDirection: "row", flexWrap: "wrap", gap: 8 },
  filter: {
    paddingVertical: 10,
    paddingHorizontal: 12,
    borderWidth: 1,
    borderRadius: 8,
  },
  empty: { padding: 20, fontSize: 14 },
});
