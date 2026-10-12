import { Stack, router, useLocalSearchParams } from "expo-router";
import { useEffect, useMemo, useState } from "react";
import {
  AppState,
  FlatList,
  RefreshControl,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  portfolioId,
  type GatewayRemoteQuery,
  type RemoteExecutionCursor,
  type RemoteExecutionKind,
} from "@waku/client";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { AutomationChoice } from "@/components/automation-form-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import {
  gatewayPairingIdentity,
  remoteTaskDestination,
} from "@/lib/gateway-remote";
const kinds: [RemoteExecutionKind | null, string][] = [
  [null, "All types"],
  ["model", "Model calls"],
  ["command", "Commands"],
  ["gpu", "GPU jobs"],
];
const initial = () => ({
  projectId: null as string | null,
  kind: null as RemoteExecutionKind | null,
  unresolvedOnly: true,
  after: null as RemoteExecutionCursor | null,
  previous: [] as (RemoteExecutionCursor | null)[],
});
export default function GatewayRemoteExecutionsScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    params = useLocalSearchParams<{ owner?: string }>();
  const invalidOwner =
    params.owner !== undefined &&
    (typeof params.owner !== "string" || !portfolioId(params.owner));
  const [filters, setFilters] = useState(initial);
  const query = useMemo<GatewayRemoteQuery>(
    () => ({
      kind: "remoteExecutions",
      query: {
        projectId: filters.projectId,
        kind: filters.kind,
        unresolvedOnly: filters.unresolvedOnly,
        after: filters.after,
        limit: 32,
      },
    }),
    [filters],
  );
  const state = useGatewayPortfolio(query, {
    ownerId: typeof params.owner === "string" ? params.owner : undefined,
    enabled: !invalidOwner,
  });
  const identity = JSON.stringify([
    gatewayPairingIdentity(state.pairing),
    state.ownerId,
  ]);
  const [pagingIdentity, setPagingIdentity] = useState("");
  useEffect(() => {
    setPagingIdentity(identity);
    setFilters(initial());
  }, [identity]);
  const result =
    !invalidOwner &&
    pagingIdentity === identity &&
    state.result?.kind === "remoteExecutions"
      ? state.result
      : null;
  const entries = result?.page.entries ?? [];
  const projects = new Map(state.projects.projects.map((p) => [p.id, p.name]));
  for (const row of entries)
    if (!projects.has(row.projectId))
      projects.set(row.projectId, `Project ${row.projectId.slice(0, 8)}`);
  if (filters.projectId && !projects.has(filters.projectId))
    projects.set(filters.projectId, `Project ${filters.projectId.slice(0, 8)}`);
  const change = (value: Partial<typeof filters>) =>
    setFilters({ ...filters, ...value, after: null, previous: [] });
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <Stack.Screen options={{ title: "Remote executions" }} />
      <FlatList
        data={entries}
        keyExtractor={(row) => `${row.kind}:${row.requestId}`}
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
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              Remote executions
            </Text>
            <AutomationRefresh
              refreshing={state.loading}
              onRefresh={() => void state.refresh()}
            />
            <AutomationChoice
              label="Owner"
              value={state.ownerId}
              options={state.owners.map((o) => ({
                value: o.publisherId,
                label: o.name,
              }))}
              disabled={
                !!params.owner ||
                invalidOwner ||
                state.loading ||
                !state.owners.length
              }
              onChange={(value) => {
                setFilters(initial());
                state.selectOwner(value);
              }}
            />
            <AutomationChoice
              label="Project"
              value={filters.projectId ?? ""}
              options={[
                { value: "", label: "All projects" },
                ...[...projects].map(([value, label]) => ({ value, label })),
              ]}
              disabled={state.loading}
              onChange={(value) => change({ projectId: value || null })}
            />
            {state.projects.nextCursor && (
              <AutomationButton
                label="More projects"
                disabled={state.loading}
                onPress={() => void state.moreProjects()}
              />
            )}
            <AutomationChoice
              label="Type"
              value={filters.kind ?? ""}
              options={kinds.map(([value, label]) => ({
                value: value ?? "",
                label,
              }))}
              disabled={state.loading}
              onChange={(value) =>
                change({ kind: (value || null) as RemoteExecutionKind | null })
              }
            />
            <AutomationChoice
              label="Records"
              value={filters.unresolvedOnly ? "unresolved" : "all"}
              options={[
                { value: "unresolved", label: "Unresolved" },
                { value: "all", label: "All records" },
              ]}
              disabled={state.loading}
              onChange={(value) =>
                change({ unresolvedOnly: value === "unresolved" })
              }
            />
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              Saved remote receipts. Stop requests, provider termination and GPU
              cleanup remain separate.
            </Text>
            {result && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Updated {new Date(result.servedAt).toLocaleString()}
                {result.pageTruncated ? " · More records on the next page" : ""}
              </Text>
            )}
            {(invalidOwner || state.message) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.text }]}
              >
                {invalidOwner
                  ? "Open remote executions from an owner Usage view."
                  : state.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {invalidOwner || state.message
              ? ""
              : state.loading
                ? "Loading remote executions…"
                : result
                  ? "No remote executions match these filters."
                  : "Connect and refresh to read remote executions."}
          </Text>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text
              accessibilityRole="header"
              style={[styles.heading, { color: theme.text }]}
            >
              {item.teamName} ·{" "}
              {item.kind === "gpu"
                ? "GPU job"
                : item.kind === "command"
                  ? "Command"
                  : "Model call"}
            </Text>
            <Text style={[styles.body, { color: theme.text }]}>
              {item.objective}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {projects.get(item.projectId) ?? "Project"} · {item.state}
              {item.stopRequested ? " · Stop requested" : ""}
              {item.settled ? " · Recorded terminal observation" : ""}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {item.operationAvailable
                ? "Original operation recorded"
                : item.jobAvailable
                  ? "Original job recorded"
                  : item.settled
                    ? "No active operation recorded"
                    : "Original handle unavailable; outcome remains unresolved"}
              {item.cancellationAvailable
                ? " · Cancellation operation recorded"
                : ""}
              {item.cleanupState ? ` · Cleanup ${item.cleanupState}` : ""}
            </Text>
            <AutomationButton
              label="Open team task"
              disabled={
                state.loading || !state.pairing?.grants.includes("taskRead")
              }
              onPress={() => {
                if (
                  !state.pairing ||
                  state.loading ||
                  AppState.currentState !== "active"
                )
                  return;
                router.push(
                  remoteTaskDestination(item, state.ownerId, state.pairing),
                );
              }}
            />
          </View>
        )}
        ListFooterComponent={
          <View style={styles.footer}>
            {filters.previous.length > 0 && (
              <AutomationButton
                label="Previous page"
                disabled={state.loading || !result}
                onPress={() =>
                  setFilters({
                    ...filters,
                    after: filters.previous.at(-1) ?? null,
                    previous: filters.previous.slice(0, -1),
                  })
                }
              />
            )}
            {result?.page.nextCursor && (
              <AutomationButton
                label="Next page"
                disabled={state.loading}
                onPress={() =>
                  setFilters({
                    ...filters,
                    after: result.page.nextCursor,
                    previous: [...filters.previous, filters.after],
                  })
                }
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
  title: { fontSize: 22, lineHeight: 30, fontWeight: "600" },
  body: { fontSize: 16, lineHeight: 24 },
  heading: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  row: { padding: 20, gap: 10, borderBottomWidth: StyleSheet.hairlineWidth },
  empty: { padding: 20, fontSize: 16, lineHeight: 24 },
  footer: { padding: 20, gap: 10 },
});
