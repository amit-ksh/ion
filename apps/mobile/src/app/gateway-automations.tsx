import AsyncStorage from "@react-native-async-storage/async-storage";
import type {
  AutomationCommand,
  AutomationOwner,
  AutomationRunSummary,
  AutomationDefinition,
} from "@waku/client";
import * as Crypto from "expo-crypto";
import { router, Stack, useFocusEffect } from "expo-router";
import { useCallback, useMemo, useRef, useState } from "react";
import {
  Alert,
  AppState,
  FlatList,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { AutomationChoice } from "@/components/automation-form-controls";
import { useTheme } from "@/hooks/use-theme";
import {
  gatewayOwnerFresh,
  submitGatewayAutomation,
  type AutomationIntent,
} from "@/lib/gateway-automation";
import {
  gatewayAutomationCommands,
  gatewayAutomationOwners,
  type GatewayPairing,
} from "@/lib/gateway-client";
import { readGatewayPairing } from "@/lib/gateway-storage";

type Owner = AutomationOwner & { name: string };
type Row =
  | {
      type: "definition";
      record: AutomationDefinition & { fingerprint: string };
    }
  | { type: "run"; record: AutomationRunSummary & { fingerprint: string } };
const status = {
  running: "Running",
  waitingDecision: "Decision required",
  blocked: "Needs attention",
  completed: "Completed",
  canceled: "Stopped",
  skipped: "Skipped",
};
const commandStatus = {
  queued: "Queued",
  uncertain: "Outcome unknown · not replayed",
  applied: "Applied",
  rejected: "Rejected · refresh before trying again",
};
export default function GatewayAutomationsScreen() {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const [owners, setOwners] = useState<Owner[]>([]);
  const [commands, setCommands] = useState<AutomationCommand[]>([]);
  const [ownerId, setOwnerId] = useState("");
  const [project, setProject] = useState("");
  const [tab, setTab] = useState("workflows");
  const [fresh, setFresh] = useState(false);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [pairing, setPairing] = useState<GatewayPairing | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const visible = useRef(false);
  const epoch = useRef(0);
  const querying = useRef(false);
  const locked = useRef(false);
  const pairingRef = useRef(pairing);
  pairingRef.current = pairing;
  const ownersRef = useRef(owners);
  ownersRef.current = owners;
  const freshRef = useRef(fresh);
  freshRef.current = fresh;
  const lastIdentity = useRef("");
  async function refresh() {
    if (
      !visible.current ||
      AppState.currentState !== "active" ||
      querying.current
    )
      return;
    querying.current = true;
    const generation = epoch.current;
    setLoading(true);
    const alive = () =>
      visible.current &&
      generation === epoch.current &&
      AppState.currentState === "active";
    try {
      const current = await readGatewayPairing();
      if (!alive()) return;
      const identity = current
        ? `${current.baseUrl}:${current.id}:${current.token}`
        : "";
      if (identity !== lastIdentity.current) {
        lastIdentity.current = identity;
        epoch.current += 1;
        setOwners([]);
        setCommands([]);
        setOwnerId("");
        setProject("");
        setPairing(current);
        pairingRef.current = current;
        setFresh(false);
        freshRef.current = false;
        // Restart with the new identity, after this serialized refresh finishes.
        return;
      }
      // Keep a stable reference while the credential identity is unchanged.
      const credential = pairingRef.current ?? current;
      setPairing(credential);
      pairingRef.current = credential;
      if (
        !current ||
        !current.grants.includes("automationRead") ||
        current.expiresAt <= Date.now()
      ) {
        setFresh(false);
        freshRef.current = false;
        setOwners([]);
        setCommands([]);
        setMessage(
          "Pair this phone with Read workflows in the gateway owner console. Control workflows enables run and schedule actions.",
        );
        return;
      }
      const [feed, history] = await Promise.all([
        gatewayAutomationOwners(current),
        current.grants.includes("automationControl")
          ? gatewayAutomationCommands(current)
          : Promise.resolve([]),
      ]);
      if (!alive()) return;
      setOwners(feed);
      ownersRef.current = feed;
      setCommands(history);
      setFresh(true);
      freshRef.current = true;
      setOwnerId((selected) =>
        feed.some((owner) => owner.publisherId === selected)
          ? selected
          : (feed[0]?.publisherId ?? ""),
      );
      setMessage(
        feed.length
          ? null
          : "No registered desktop has published workflows yet.",
      );
    } catch (error) {
      if (alive()) {
        setFresh(false);
        freshRef.current = false;
        setMessage(
          error instanceof Error
            ? error.message
            : "Workflow feed unavailable. Controls are paused.",
        );
      }
    } finally {
      querying.current = false;
      if (visible.current) setLoading(false);
      if (
        visible.current &&
        generation !== epoch.current &&
        AppState.currentState === "active"
      )
        void refresh();
    }
  }
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  useFocusEffect(
    useCallback(() => {
      visible.current = true;
      epoch.current += 1;
      setFresh(false);
      freshRef.current = false;
      void refreshRef.current();
      const timer = setInterval(() => void refreshRef.current(), 8_000);
      const subscription = AppState.addEventListener("change", (state) => {
        epoch.current += 1;
        setFresh(false);
        freshRef.current = false;
        if (state === "active") void refreshRef.current();
      });
      return () => {
        visible.current = false;
        epoch.current += 1;
        freshRef.current = false;
        clearInterval(timer);
        subscription.remove();
      };
    }, []),
  );
  const owner = owners.find((item) => item.publisherId === ownerId);
  const canControl =
    fresh &&
    visible.current &&
    owner &&
    gatewayOwnerFresh(owner) &&
    pairing?.grants.includes("automationControl") &&
    pairing.expiresAt > Date.now();
  const names = useMemo(
    () => new Map(owner?.projects.map((p) => [p.id, p.name]) ?? []),
    [owner],
  );
  const rows: Row[] = useMemo(
    () =>
      owner
        ? tab === "runs"
          ? owner.runs
              .filter((r) => !project || r.projectId === project)
              .map((record) => ({ type: "run", record }))
          : owner.definitions
              .filter(
                (r) =>
                  r.kind === (tab === "routines" ? "routine" : "workflow") &&
                  (!project || r.projectId === project),
              )
              .map((record) => ({ type: "definition", record }))
        : [],
    [owner, tab, project],
  );
  function confirm(
    intent: AutomationIntent,
    fingerprint: string,
    name: string,
  ) {
    const selected = owner;
    const current = pairingRef.current;
    const generation = epoch.current;
    if (!selected || !current || !canControl || locked.current) return;
    const label =
      intent.kind === "start"
        ? "Run now"
        : intent.kind === "stop"
          ? "Stop run"
          : intent.kind === "routine"
            ? intent.paused
              ? "Pause schedule"
              : "Enable schedule"
            : intent.approved
              ? "Approve stage"
              : "Decline stage";
    Alert.alert(
      `${label}?`,
      `${name}\nOwner: ${selected.name}\n${intent.kind === "gate" && intent.approved ? "Start this stage within its configured team access and budget. App actions keep their own approval requirements." : "Use the saved definition and configured team access and budget on this owner."}`,
      [
        { text: "Cancel", style: "cancel" },
        {
          text: label,
          style:
            intent.kind === "stop" ||
            (intent.kind === "gate" && !intent.approved)
              ? "destructive"
              : "default",
          onPress: () => void act(),
        },
      ],
    );
    async function act() {
      if (
        locked.current ||
        !visible.current ||
        AppState.currentState !== "active" ||
        generation !== epoch.current ||
        !freshRef.current ||
        pairingRef.current !== current
      )
        return;
      const latest = ownersRef.current.find(
        (o) => o.publisherId === selected!.publisherId,
      );
      const resource =
        "definitionId" in intent
          ? latest?.definitions.find((d) => d.id === intent.definitionId)
          : latest?.runs.find((r) => r.id === intent.runId);
      if (
        !latest ||
        !gatewayOwnerFresh(latest) ||
        resource?.fingerprint !== fingerprint
      ) {
        setMessage("The workflow changed. Refresh before deciding.");
        return;
      }
      locked.current = true;
      setBusy(true);
      const alive = () =>
        visible.current &&
        AppState.currentState === "active" &&
        generation === epoch.current &&
        pairingRef.current === current;
      try {
        const saved = await readGatewayPairing();
        if (
          !alive() ||
          !saved ||
          saved.id !== current!.id ||
          saved.baseUrl !== current!.baseUrl ||
          saved.token !== current!.token
        )
          return;
        const result = await submitGatewayAutomation(
          current!,
          AsyncStorage,
          selected!.publisherId,
          fingerprint,
          intent,
          Crypto.randomUUID,
          undefined,
          alive,
        );
        if (!alive()) return;
        setMessage(
          `${result.recovered ? "Earlier request" : "Request"}: ${commandStatus[result.command.status]}.`,
        );
        setCommands((items) =>
          [
            result.command,
            ...items.filter((c) => c.id !== result.command.id),
          ].slice(0, 32),
        );
        void refreshRef.current();
      } catch (error) {
        if (alive())
          setMessage(
            error instanceof Error
              ? error.message
              : "Request outcome unknown. Refresh and recover before sending another action.",
          );
      } finally {
        locked.current = false;
        if (visible.current) setBusy(false);
      }
    }
  }
  function waiting(kind: AutomationIntent["kind"], id: string): boolean {
    return commands.some(
      (c) =>
        c.publisherId === ownerId &&
        c.action.kind === kind &&
        ("definitionId" in c.action
          ? c.action.definitionId
          : c.action.runId) === id &&
        ["queued", "uncertain"].includes(c.status),
    );
  }
  return (
    <FlatList
      style={{ backgroundColor: theme.background }}
      contentContainerStyle={[
        styles.page,
        { paddingBottom: insets.bottom + 28 },
      ]}
      data={rows}
      keyExtractor={(row) => `${row.type}:${row.record.id}`}
      renderItem={({ item }) => {
        const r = item.record;
        return (
          <View
            style={[
              styles.card,
              { borderColor: theme.border, backgroundColor: theme.surface },
            ]}
          >
            <Text style={[styles.meta, { color: theme.textSecondary }]}>
              {names.get(r.projectId) ?? "Project"}
            </Text>
            <Text style={[styles.heading, { color: theme.text }]}>
              {r.name}
            </Text>
            {item.type === "definition" ? (
              <>
                <Text style={[styles.meta, { color: theme.textSecondary }]}>
                  {item.record.stageCount} stages
                  {item.record.kind === "routine"
                    ? ` · ${item.record.paused ? "Paused" : "Enabled"}`
                    : ""}
                </Text>
                {item.record.schedule && (
                  <Text style={[styles.meta, { color: theme.textSecondary }]}>
                    {item.record.schedule}
                  </Text>
                )}
                {!!item.record.nextRun && (
                  <Text style={[styles.meta, { color: theme.textSecondary }]}>
                    Next:{" "}
                    {new Date(item.record.nextRun * 1000).toLocaleString()}
                  </Text>
                )}
                <View style={styles.actions}>
                  <AutomationButton
                    label={
                      waiting("start", r.id) ? "Recover run request" : "Run now"
                    }
                    disabled={!canControl || busy || !item.record.ready}
                    onPress={() =>
                      confirm(
                        {
                          kind: "start",
                          definitionId: r.id,
                          expectedRevision: item.record.revision,
                        },
                        r.fingerprint,
                        r.name,
                      )
                    }
                  />
                  {item.record.kind === "routine" && (
                    <AutomationButton
                      label={
                        waiting("routine", r.id)
                          ? "Recover schedule request"
                          : item.record.paused
                            ? "Enable schedule"
                            : "Pause schedule"
                      }
                      disabled={
                        !canControl ||
                        busy ||
                        (!!item.record.paused && !item.record.ready)
                      }
                      onPress={() =>
                        confirm(
                          {
                            kind: "routine",
                            definitionId: r.id,
                            expectedRevision: item.record.revision,
                            paused: !item.record.paused,
                          },
                          r.fingerprint,
                          r.name,
                        )
                      }
                    />
                  )}
                </View>
              </>
            ) : (
              <>
                <Text style={[styles.meta, { color: theme.textSecondary }]}>
                  {status[item.record.status]} · Stage{" "}
                  {Math.min(
                    item.record.currentStage + 1,
                    item.record.stageCount,
                  )}{" "}
                  of {item.record.stageCount}
                </Text>
                {item.record.gate && (
                  <>
                    <Text style={[styles.heading, { color: theme.text }]}>
                      {item.record.gate.title}
                    </Text>
                    <Text
                      selectable
                      style={[styles.meta, { color: theme.text }]}
                    >
                      {item.record.gate.instructions}
                    </Text>
                    {!item.record.gate.reviewComplete && (
                      <Text style={[styles.meta, { color: theme.warning }]}>
                        Full stage instructions require desktop review.
                      </Text>
                    )}
                    <View style={styles.actions}>
                      <AutomationButton
                        label="Decline stage"
                        destructive
                        disabled={!canControl || busy}
                        onPress={() =>
                          confirm(
                            {
                              kind: "gate",
                              runId: r.id,
                              expectedRevision: item.record.revision,
                              stage: item.record.gate!.index,
                              approved: false,
                            },
                            r.fingerprint,
                            r.name,
                          )
                        }
                      />
                      <AutomationButton
                        label={
                          waiting("gate", r.id)
                            ? "Recover stage request"
                            : "Approve stage"
                        }
                        disabled={
                          !canControl ||
                          busy ||
                          !item.record.gate.reviewComplete
                        }
                        onPress={() =>
                          confirm(
                            {
                              kind: "gate",
                              runId: r.id,
                              expectedRevision: item.record.revision,
                              stage: item.record.gate!.index,
                              approved: true,
                            },
                            r.fingerprint,
                            r.name,
                          )
                        }
                      />
                    </View>
                  </>
                )}
                <AutomationButton
                  label="Run details & documents"
                  onPress={() =>
                    router.push({
                      pathname: "/gateway-workflow/[id]",
                      params: { id: r.id, owner: ownerId },
                    })
                  }
                />
                {!["completed", "canceled", "skipped"].includes(
                  item.record.status,
                ) && (
                  <AutomationButton
                    label={
                      waiting("stop", r.id)
                        ? "Recover stop request"
                        : "Stop run"
                    }
                    destructive
                    disabled={!canControl || busy}
                    onPress={() =>
                      confirm(
                        {
                          kind: "stop",
                          runId: r.id,
                          expectedRevision: item.record.revision,
                        },
                        r.fingerprint,
                        r.name,
                      )
                    }
                  />
                )}
              </>
            )}
          </View>
        );
      }}
      ListHeaderComponent={
        <View style={styles.header}>
          <Stack.Screen options={{ title: "Gateway workflows" }} />
          <View style={styles.titleRow}>
            <Text style={[styles.title, { color: theme.text }]}>
              Workflows & routines
            </Text>
            <AutomationRefresh
              refreshing={loading || busy}
              onRefresh={() => void refreshRef.current()}
            />
          </View>
          {message && (
            <Text
              accessibilityRole="alert"
              style={[styles.meta, { color: theme.textSecondary }]}
            >
              {message}
            </Text>
          )}
          <AutomationButton
            label="Workflow history"
            disabled={!ownerId}
            onPress={() =>
              router.push({
                pathname: "/gateway-workflow-history",
                params: { owner: ownerId },
              })
            }
          />
          {owners.length > 0 && (
            <>
              <AutomationChoice
                label="Owner"
                value={ownerId}
                options={owners.map((o) => ({
                  value: o.publisherId,
                  label: o.name,
                }))}
                disabled={busy}
                onChange={(value) => {
                  setOwnerId(value);
                  setProject("");
                }}
              />
              <Text style={[styles.meta, { color: theme.textSecondary }]}>
                {fresh && owner && gatewayOwnerFresh(owner)
                  ? "Connected owner"
                  : "Owner offline or stale · controls paused"}
              </Text>
              {!pairing?.grants.includes("automationControl") && (
                <Text style={[styles.meta, { color: theme.textSecondary }]}>
                  Read only · pair with Control workflows to run or change
                  schedules.
                </Text>
              )}
              {owner?.truncated && (
                <Text style={[styles.meta, { color: theme.textSecondary }]}>
                  Recent runs and pending decisions shown. Open Workflow history
                  to browse every run.
                </Text>
              )}
              <AutomationChoice
                label="Project"
                value={project}
                options={[
                  { value: "", label: "All projects" },
                  ...(owner?.projects.map((p) => ({
                    value: p.id,
                    label: p.name,
                  })) ?? []),
                ]}
                disabled={busy}
                onChange={setProject}
              />
              <AutomationChoice
                label="View"
                value={tab}
                options={[
                  { value: "workflows", label: "Workflows" },
                  { value: "routines", label: "Routines" },
                  { value: "runs", label: "Runs & decisions" },
                ]}
                disabled={busy}
                onChange={setTab}
              />
            </>
          )}
        </View>
      }
      ListEmptyComponent={
        <Text style={[styles.meta, { color: theme.textSecondary }]}>
          No workflows or runs in this view.
        </Text>
      }
      ListFooterComponent={
        commands.length > 0 ? (
          <View style={styles.header}>
            <Text style={[styles.heading, { color: theme.text }]}>
              Recent requests
            </Text>
            {commands
              .filter((c) => c.publisherId === ownerId)
              .map((c) => (
                <View
                  key={c.id}
                  style={[styles.receipt, { borderColor: theme.border }]}
                >
                  <Text style={[styles.meta, { color: theme.text }]}>
                    {
                      {
                        start: "Start run",
                        stop: "Stop run",
                        gate: "Stage decision",
                        routine: "Schedule change",
                      }[c.action.kind]
                    }{" "}
                    · {commandStatus[c.status]}
                  </Text>
                  <Text
                    selectable
                    style={[styles.meta, { color: theme.textSecondary }]}
                  >
                    Request: {c.id}
                  </Text>
                  <Text
                    selectable
                    style={[styles.meta, { color: theme.textSecondary }]}
                  >
                    {"definitionId" in c.action ? "Definition" : "Run"}:{" "}
                    {"definitionId" in c.action
                      ? c.action.definitionId
                      : c.action.runId}
                  </Text>
                </View>
              ))}
          </View>
        ) : null
      }
    />
  );
}
const styles = StyleSheet.create({
  page: {
    padding: 20,
    gap: 16,
    width: "100%",
    maxWidth: 760,
    alignSelf: "center",
  },
  header: { gap: 16 },
  titleRow: { flexDirection: "row", alignItems: "center", gap: 12 },
  title: { fontSize: 24, lineHeight: 32, fontWeight: "600", flex: 1 },
  heading: { fontSize: 17, lineHeight: 25, fontWeight: "600" },
  meta: { fontSize: 14, lineHeight: 22 },
  card: { padding: 16, borderWidth: 1, borderRadius: 12, gap: 10 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  receipt: { borderTopWidth: StyleSheet.hairlineWidth, paddingTop: 12, gap: 6 },
});
