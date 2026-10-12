import { router } from "expo-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { ArtifactFilter } from "@/components/artifact-filter";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { ConnectionBanner } from "@/components/connection-banner";
import { useScreenHeaderInset } from "@/components/screen-header";
import { useAutomationQuery } from "@/hooks/use-automation-query";
import { useTheme } from "@/hooks/use-theme";
import { ARTIFACT_FORMATS, loadArtifactLibrary } from "@/lib/artifact-library";
import { loadTaskState } from "@/lib/daemon-api";
import { routineTime } from "@/lib/automation-api";
import type { WakuClient } from "@waku/client";
const NAMES: Record<string, string> = {
  md: "Markdown",
  txt: "Text",
  json: "JSON",
  csv: "CSV",
  html: "HTML",
  htm: "HTML (HTM)",
  pdf: "PDF",
  docx: "Word",
};
export default function ArtifactsScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    header = useScreenHeaderInset();
  const [project, setProject] = useState<string | null>(null),
    [task, setTask] = useState<string | null>(null),
    [format, setFormat] = useState<string | null>(null),
    [after, setAfter] = useState<string | null>(null),
    [previous, setPrevious] = useState<(string | null)[]>([]);
  const load = useCallback(
    async (client: WakuClient) => {
      const [page, directory] = await Promise.all([
        loadArtifactLibrary(client, {
          projectId: project,
          taskId: task,
          format,
          after,
          limit: 32,
        }),
        loadTaskState(client),
      ]);
      return { page, directory };
    },
    [project, task, format, after],
  );
  const state = useAutomationQuery(
    ["artifacts", project, task, format, after],
    load,
    { poll: false },
  );
  useEffect(() => {
    setProject(null);
    setTask(null);
    setFormat(null);
    setAfter(null);
    setPrevious([]);
  }, [state.daemon.activeProfile?.id, state.daemon.activeProfile?.address]);
  const data = state.fresh && !state.query.isFetching ? state.data : undefined;
  const projects = useMemo(
    () =>
      (data?.directory.projects ?? []).map((item) => ({
        id: item.id,
        name: item.name,
      })),
    [data?.directory.projects],
  );
  const tasks = useMemo(
    () =>
      (data?.directory.sessions ?? [])
        .filter((item) => !project || item.project_id === project)
        .map((item) => ({
          id: item.id,
          name: item.auto_title || item.title || "Task",
        })),
    [data?.directory.sessions, project],
  );
  const projectNames = useMemo(
    () => new Map(projects.map((item) => [item.id, item.name])),
    [projects],
  );
  const taskNames = useMemo(
    () => new Map(tasks.map((item) => [item.id, item.name])),
    [tasks],
  );
  const busy = !state.fresh || state.query.isFetching;
  function refresh() {
    setAfter(null);
    setPrevious([]);
    if (after === null) void state.query.refetch();
  }
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <FlatList
        data={data?.page.items ?? []}
        keyExtractor={(item) => item.id}
        initialNumToRender={12}
        maxToRenderPerBatch={12}
        contentContainerStyle={{
          paddingTop: header + 8,
          paddingBottom: insets.bottom + 24,
        }}
        refreshControl={
          <RefreshControl
            refreshing={state.query.isFetching}
            onRefresh={refresh}
            tintColor={theme.textSecondary}
          />
        }
        ListHeaderComponent={
          <View style={styles.header}>
            <ConnectionBanner />
            <AutomationRefresh
              refreshing={
                state.query.isFetching || state.daemon.phase !== "connected"
              }
              onRefresh={refresh}
            />
            <View style={styles.actions}>
              <ArtifactFilter
                label="Projects"
                value={project}
                choices={projects}
                disabled={busy}
                onChange={(id) => {
                  setProject(id);
                  setTask(null);
                  setAfter(null);
                  setPrevious([]);
                }}
              />
              <ArtifactFilter
                label="Tasks"
                value={task}
                choices={tasks}
                disabled={busy}
                onChange={(id) => {
                  setTask(id);
                  setAfter(null);
                  setPrevious([]);
                }}
              />
              <ArtifactFilter
                label="Formats"
                value={format}
                choices={ARTIFACT_FORMATS.map((id) => ({
                  id,
                  name: NAMES[id]!,
                }))}
                disabled={busy}
                onChange={(id) => {
                  setFormat(id);
                  setAfter(null);
                  setPrevious([]);
                }}
              />
            </View>
            {data && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {data.page.total} agent-published documents
              </Text>
            )}
            {state.query.error && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {state.query.error.message}
              </Text>
            )}
            {!!after && (
              <AutomationButton label="Reset page" onPress={refresh} />
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.query.isFetching
              ? "Loading documents…"
              : state.daemon.phase !== "connected"
                ? "Connect to the owner daemon to view artifacts."
                : state.query.error
                  ? "Refresh the library to retry."
                  : "No agent-published documents match these filters."}
          </Text>
        }
        ListFooterComponent={
          <View style={[styles.actions, { padding: 20 }]}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous"
                disabled={busy}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {data?.page.nextCursor && (
              <AutomationButton
                label="Next"
                disabled={busy}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(data.page.nextCursor);
                }}
              />
            )}
          </View>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              {item.name}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {NAMES[item.format]} · {item.bytes.toLocaleString()} bytes ·{" "}
              {routineTime(item.createdAt)}
            </Text>
            <Text
              numberOfLines={2}
              style={[styles.body, { color: theme.textSecondary }]}
            >
              {projectNames.get(item.projectId) ?? "Project"} ·{" "}
              {taskNames.get(item.taskId) ?? "Task"}
            </Text>
            <AutomationButton
              label={`Open ${item.name}`}
              disabled={busy}
              onPress={() => {
                if (!data || !state.fresh || !state.daemon.activeProfile)
                  return;
                router.push({
                  pathname: "/artifact-document",
                  params: {
                    owner: state.daemon.activeProfile.id,
                    document: JSON.stringify(item),
                    address: state.daemon.activeProfile.address,
                  },
                });
              }}
            />
          </View>
        )}
      />
    </View>
  );
}
const styles = StyleSheet.create({
  header: { paddingHorizontal: 20, gap: 12, paddingBottom: 20 },
  row: {
    paddingHorizontal: 20,
    paddingVertical: 20,
    borderBottomWidth: StyleSheet.hairlineWidth,
    gap: 10,
  },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  title: { fontSize: 17, fontWeight: "600", lineHeight: 24 },
  body: { fontSize: 14, lineHeight: 21 },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
