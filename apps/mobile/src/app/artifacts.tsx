import { router } from "expo-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import { AppSymbol } from "@/components/app-symbol";
import { ArtifactFilter } from "@/components/artifact-filter";
import { TicketCard } from "@/components/ticket-card";
import { WorkbenchState } from "@/components/workbench";
import { relativeSessionTime } from "@/lib/session-presentation";
import { ticketNumber } from "@/lib/ticket";
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
import { localTimeLabel } from "@/lib/time-labels";
import type { WakuClient } from "@waku/client";
// Typed against the format list so a new format cannot ship without a name;
// a missing one used to crash the Formats filter.
const NAMES: Record<(typeof ARTIFACT_FORMATS)[number], string> = {
  md: "Markdown",
  txt: "Text",
  json: "JSON",
  csv: "CSV",
  html: "HTML",
  htm: "HTML (HTM)",
  pdf: "PDF",
  docx: "Word",
  xlsx: "Excel",
  pptx: "PowerPoint",
  png: "PNG image",
  svg: "SVG image",
  xml: "XML",
  yaml: "YAML",
  yml: "YAML (YML)",
};
const formatName = (format: string) => NAMES[format as keyof typeof NAMES] ?? format.toUpperCase();
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
        contentContainerStyle={[styles.list, {
          paddingTop: header + 8,
          paddingBottom: insets.bottom + 24,
        }]}
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
            <View style={styles.summary}>
              <Text style={[styles.body, styles.count, { color: theme.textSecondary }]}>
                {data ? `${data.page.total} agent-published documents` : " "}
              </Text>
              <AutomationRefresh
                refreshing={
                  state.query.isFetching || state.daemon.phase !== "connected"
                }
                onRefresh={refresh}
              />
            </View>
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
                  name: NAMES[id],
                }))}
                disabled={busy}
                onChange={(id) => {
                  setFormat(id);
                  setAfter(null);
                  setPrevious([]);
                }}
              />
            </View>
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
          state.query.isFetching ? (
            <WorkbenchState title="Loading documents…" />
          ) : state.daemon.phase !== "connected" ? (
            <WorkbenchState
              icon={{ android: "link_off", web: "link_off" }}
              title="Connect to your desktop"
              detail="Connect to the owner daemon to view artifacts."
            />
          ) : state.query.error ? (
            <WorkbenchState
              icon={{ android: "cloud_off", web: "cloud_off" }}
              title="Could not load documents"
              detail="Refresh the library to retry."
            />
          ) : (
            <WorkbenchState
              icon={{ android: "description", web: "description" }}
              title="No documents yet"
              detail="No agent-published documents match these filters."
            />
          )
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
          <View style={styles.row}>
            <TicketCard
              number={ticketNumber(item.id)}
              time={relativeSessionTime(item.createdAt)}
              title={item.name}
              mark={<AppSymbol name={FORMAT_ICON} size={15} tintColor={theme.textSecondary} />}
              station={`${projectNames.get(item.projectId) ?? "Project"} · ${taskNames.get(item.taskId) ?? "Task"}`}
              status={{ label: `${formatName(item.format)} · ${formatBytes(item.bytes)}`, icon: FORMAT_ICON, tone: "neutral" }}
              accessibilityLabel={`Open ${item.name}`}
              accessibilityHint={`${formatName(item.format)}, ${formatBytes(item.bytes)}, ${localTimeLabel(item.createdAt)}`}
              onPress={() => {
                if (busy || !data || !state.fresh || !state.daemon.activeProfile)
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
const FORMAT_ICON = { ios: "doc.text", android: "description", web: "description" } as const;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(bytes < 10_240 ? 1 : 0)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

const styles = StyleSheet.create({
  list: { width: "100%", maxWidth: 1000, alignSelf: "center" },
  header: { paddingHorizontal: 16, gap: 10, paddingBottom: 16 },
  row: { paddingHorizontal: 16, paddingBottom: 12 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 4 },
  summary: { flexDirection: "row", alignItems: "center", gap: 8 },
  count: { flex: 1, fontVariant: ["tabular-nums"] },
  body: { fontSize: 14, lineHeight: 21 },
});
