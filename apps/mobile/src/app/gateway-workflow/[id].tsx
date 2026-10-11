import { portfolioId, type PortfolioQuery } from "@waku/client";
import { router, Stack, useLocalSearchParams } from "expo-router";
import { useMemo } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import { automationStatus, routineTime } from "@/lib/automation-api";
export default function GatewayWorkflowRunScreen() {
  const params = useLocalSearchParams<{ id: string; owner: string }>(),
    theme = useTheme(),
    insets = useSafeAreaInsets();
  const valid =
    typeof params.id === "string" &&
    portfolioId(params.id) &&
    typeof params.owner === "string" &&
    portfolioId(params.owner);
  const query = useMemo<PortfolioQuery>(
    () => ({
      kind: "workflowRun",
      query: { runId: typeof params.id === "string" ? params.id : "" },
    }),
    [params.id],
  );
  const state = useGatewayPortfolio(query, {
    ownerId: typeof params.owner === "string" ? params.owner : "",
    loadProjects: false,
  });
  const page =
    valid && state.result?.kind === "workflowRun" ? state.result.page : null;
  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ title: page?.run.name ?? "Workflow run" }} />
      <FlatList
        data={page?.stages ?? []}
        keyExtractor={(s) => s.teamRunId}
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
            {page && (
              <>
                <Text style={[styles.title, { color: theme.text }]}>
                  {page.run.name}
                </Text>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {automationStatus(page.run.status)} · Created{" "}
                  {routineTime(page.run.createdAt)} · Deadline{" "}
                  {routineTime(page.run.deadline)}
                </Text>
                <Text
                  selectable
                  style={[styles.body, { color: theme.textSecondary }]}
                >
                  Run {page.run.id}
                </Text>
                <AutomationButton
                  label="Accepted documents"
                  disabled={!state.pairing?.grants.includes("evidenceRead")}
                  onPress={() =>
                    router.push({
                      pathname: "/gateway-workflow-evidence",
                      params: { run: page.run.id, owner: state.ownerId },
                    })
                  }
                />
                {!state.pairing?.grants.includes("evidenceRead") && (
                  <Text style={[styles.body, { color: theme.textSecondary }]}>
                    Pair with Read workflow documents to read or export
                    generated evidence.
                  </Text>
                )}
              </>
            )}
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              History is read only. Use Workflows & routines for current run
              decisions and controls.
            </Text>
            {(state.message || !valid) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {valid
                  ? state.message
                  : "Open a run from a registered owner’s workflow history."}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.loading
              ? "Loading run…"
              : "No run detail available. Refresh to retry."}
          </Text>
        }
        renderItem={({ item: s }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.heading, { color: theme.text }]}>
              {s.title}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {s.skipped
                ? "Skipped"
                : s.completed
                  ? "Completed"
                  : s.started
                    ? "Started"
                    : s.requiresDecision && !s.approved
                      ? "Decision required"
                      : "Not started"}
            </Text>
            {s.workspaceFrom && (
              <Text
                selectable
                style={[styles.body, { color: theme.textSecondary }]}
              >
                Code inherited from team run {s.workspaceFrom}
              </Text>
            )}
            {s.route && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Next:{" "}
                {s.route.target.type === "finish"
                  ? "Finish"
                  : `Stage ${s.route.target.index + 1}`}
                {s.route.matched === null
                  ? ""
                  : s.route.matched
                    ? " · Condition matched"
                    : " · Condition did not match"}
              </Text>
            )}
            {s.route?.artifactId && (
              <Text
                selectable
                style={[styles.body, { color: theme.textSecondary }]}
              >
                Routing document {s.route.artifactId} · SHA-256 {s.route.sha256}
              </Text>
            )}
            {s.review && (
              <>
                <Text selectable style={[styles.body, { color: theme.text }]}>
                  {s.review.instructions}
                </Text>
                {!s.review.complete && (
                  <Text style={[styles.body, { color: theme.textSecondary }]}>
                    Review preview shortened. Review the full stage on the
                    owner.
                  </Text>
                )}
              </>
            )}
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              Task {s.taskId}
            </Text>
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
  title: { fontSize: 22, lineHeight: 30, fontWeight: "600" },
  heading: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  body: { fontSize: 14, lineHeight: 21 },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
