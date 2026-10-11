import { portfolioId, type PortfolioQuery } from "@waku/client";
import { router, Stack, useLocalSearchParams } from "expo-router";
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
  AutomationToggle,
} from "@/components/automation-form-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import { automationStatus, routineTime } from "@/lib/automation-api";
export default function GatewayWorkflowHistoryScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets();
  const params = useLocalSearchParams<{ owner?: string }>();
  const [project, setProject] = useState(""),
    [definition, setDefinition] = useState(""),
    [draft, setDraft] = useState(""),
    [pending, setPending] = useState(false);
  const [after, setAfter] = useState<string | null>(null),
    [previous, setPrevious] = useState<(string | null)[]>([]),
    [message, setMessage] = useState<string | null>(null);
  const query = useMemo<PortfolioQuery>(
    () => ({
      kind: "workflowHistory",
      query: {
        projectId: project || null,
        definitionId: definition || null,
        pendingOnly: pending,
        after,
        limit: 32,
      },
    }),
    [project, definition, pending, after],
  );
  const state = useGatewayPortfolio(query, {
    ownerId:
      typeof params.owner === "string" && portfolioId(params.owner)
        ? params.owner
        : undefined,
  });
  const page =
    state.result?.kind === "workflowHistory" ? state.result.page : null;
  const identity = state.pairing
    ? `${state.pairing.baseUrl}:${state.pairing.id}:${state.ownerId}`
    : "";
  useEffect(() => {
    setProject("");
    setDefinition("");
    setDraft("");
    setPending(false);
    setAfter(null);
    setPrevious([]);
    setMessage(null);
  }, [identity]);
  function reset() {
    setAfter(null);
    setPrevious([]);
    setMessage(null);
  }
  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ title: "Workflow history" }} />
      <FlatList
        data={page?.runs ?? []}
        keyExtractor={(run) => run.id}
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
              options={state.owners.map((o) => ({
                value: o.publisherId,
                label: o.name,
              }))}
              disabled={!!params.owner || !state.owners.length}
              onChange={state.selectOwner}
            />
            <AutomationChoice
              label="Project"
              value={project}
              options={[
                { value: "", label: "All projects" },
                ...state.projects.projects.map((p) => ({
                  value: p.id,
                  label: p.name,
                })),
              ]}
              disabled={!state.ownerId}
              onChange={(v) => {
                setProject(v);
                reset();
              }}
            />
            {state.projects.nextCursor && (
              <AutomationButton
                label={`Load more projects (${state.projects.projects.length} of ${state.projects.total})`}
                disabled={state.loading}
                onPress={() => void state.moreProjects()}
              />
            )}
            <AutomationToggle
              label="Runs needing attention"
              value={pending}
              disabled={!state.ownerId}
              onChange={(v) => {
                setPending(v);
                reset();
              }}
            />
            <AutomationField
              label="Workflow or routine ID"
              value={draft}
              onChangeText={setDraft}
              placeholder="All definitions"
              autoCapitalize="none"
              autoCorrect={false}
            />
            <AutomationButton
              label="Apply definition filter"
              disabled={!state.ownerId}
              onPress={() => {
                const value = draft.trim().toLowerCase();
                if (value && !portfolioId(value)) {
                  setMessage(
                    "Enter a full workflow or routine ID, or leave empty for all definitions.",
                  );
                  return;
                }
                setDefinition(value);
                reset();
              }}
            />
            {page && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {page.pendingCount} stage decisions waiting in this
                project/definition scope
              </Text>
            )}
            {state.result && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Updated {new Date(state.result.servedAt).toLocaleString()}
              </Text>
            )}
            {(message || state.message) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {message ?? state.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.loading
              ? "Loading workflow history…"
              : page
                ? "No runs match these filters."
                : "Refresh to read workflow history."}
          </Text>
        }
        ListFooterComponent={
          <View style={[styles.actions, styles.footer]}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous"
                disabled={state.loading || !page}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {page?.nextCursor && (
              <AutomationButton
                label="Next"
                disabled={state.loading}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(page.nextCursor);
                }}
              />
            )}
          </View>
        }
        renderItem={({ item: run }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.title, { color: theme.text }]}>
              {run.name}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {automationStatus(run.status)} ·{" "}
              {run.currentStage < run.stageCount
                ? `Stage ${run.currentStage + 1} of ${run.stageCount}`
                : `${run.stageCount} stages`}{" "}
              · {routineTime(run.createdAt)}
            </Text>
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              Run {run.id}
            </Text>
            <AutomationButton
              label="Run details & documents"
              onPress={() =>
                router.push({
                  pathname: "/gateway-workflow/[id]",
                  params: { id: run.id, owner: state.ownerId },
                })
              }
            />
          </View>
        )}
      />
    </View>
  );
}
const styles = StyleSheet.create({
  screen: { flex: 1 },
  header: { padding: 20, gap: 12 },
  row: { padding: 20, gap: 10, borderBottomWidth: StyleSheet.hairlineWidth },
  title: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  body: { fontSize: 14, lineHeight: 21 },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  footer: { padding: 20 },
});
