import AsyncStorage from "@react-native-async-storage/async-storage";
import {
  formatBudgetAmount,
  parseBudgetAmount,
  validateCloudBudget,
  type CloudBudget,
} from "@waku/client";
import * as Crypto from "expo-crypto";
import { useEffect, useRef, useState } from "react";
import {
  Alert,
  FlatList,
  KeyboardAvoidingView,
  Platform,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import {
  AutomationField,
  AutomationToggle,
} from "@/components/automation-form-controls";
import { ConnectionBanner } from "@/components/connection-banner";
import { useScreenHeaderInset } from "@/components/screen-header";
import { useAutomationQuery } from "@/hooks/use-automation-query";
import { useTheme } from "@/hooks/use-theme";
import {
  budgetDraftKey,
  readBudgetDraft,
  saveBudgetDraft,
  type BudgetDraft,
} from "@/lib/cloud-budget-drafts";
export default function CloudBudgetsScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    header = useScreenHeaderInset();
  const state = useAutomationQuery(["cloud-budgets"], async (client) => {
    const [budgets, definitions] = await Promise.all([
      client.request({ type: "loadCloudBudgets", after: null, limit: 64 }),
      client.request({ type: "loadWorkspaceControl" }),
    ]);
    if (
      budgets.type !== "cloudBudgets" ||
      definitions.type !== "workspaceControl" ||
      budgets.page.nextCursor !== null
    )
      throw new Error("Unexpected GPU budget response.");
    return { page: budgets.page, snapshot: definitions.snapshot };
  });
  const owner = `${state.daemon.activeProfile?.id ?? ""}:${state.daemon.activeProfile?.address ?? ""}`,
    key = budgetDraftKey(owner);
  const [draft, setDraft] = useState<BudgetDraft | null>(null),
    [amount, setAmount] = useState(""),
    [targets, setTargets] = useState(""),
    [busy, setBusy] = useState(false),
    [restoring, setRestoring] = useState(true),
    [error, setError] = useState<string | null>(null);
  const mounted = useRef(true),
    locked = useRef(false),
    epoch = useRef({
      owner,
      client: state.daemon.client,
      connections: state.daemon.connections,
      visible: state.visible,
      generation: 0,
    });
  if (
    epoch.current.owner !== owner ||
    epoch.current.client !== state.daemon.client ||
    epoch.current.connections !== state.daemon.connections ||
    epoch.current.visible !== state.visible
  ) {
    epoch.current = {
      owner,
      client: state.daemon.client,
      connections: state.daemon.connections,
      visible: state.visible,
      generation: epoch.current.generation + 1,
    };
  }
  const ready = useRef(false);
  ready.current = state.fresh && state.visible && !restoring;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  function install(value: BudgetDraft | null) {
    setDraft(value);
    setAmount(value ? formatBudgetAmount(value.budget.limitMicros) : "");
    setTargets(value ? value.budget.targets.join(", ") : "");
  }
  useEffect(() => {
    let current = true;
    install(null);
    setRestoring(true);
    setError(null);
    void AsyncStorage.getItem(key)
      .then((text) => {
        if (current && text) install(readBudgetDraft(text));
      })
      .catch((e) => {
        if (current) setError(String(e));
      })
      .finally(() => {
        if (current) setRestoring(false);
      });
    return () => {
      current = false;
    };
  }, [key]);
  const disabled = !ready.current || busy;
  function start(budget?: CloudBudget) {
    if (disabled || locked.current || !state.data) return;
    const saved =
      budget &&
      state.data.snapshot.records.find(
        (r) => r.kind === "cloudBudget" && r.id === budget.id,
      );
    if (budget && !saved) {
      setError("Refresh before editing this budget.");
      return;
    }
    const record =
      saved?.kind === "cloudBudget"
        ? (({ kind: _, ...b }) => b)(saved)
        : {
            id: Crypto.randomUUID(),
            name: "",
            cloudProject: "",
            currency: "USD",
            limitMicros: 100_000_000,
            targets: [],
            paused: true,
          };
    install({
      version: 1,
      revision: state.data.snapshot.revision,
      budget: record,
    });
    setError(null);
  }
  function update(patch: Partial<CloudBudget>) {
    if (disabled || !draft) return;
    setDraft({ ...draft, budget: { ...draft.budget, ...patch } });
  }
  async function save() {
    if (disabled || locked.current || !draft || !state.daemon.client) return;
    const client = state.daemon.client,
      stamp = epoch.current.generation;
    const alive = () =>
      mounted.current && epoch.current.generation === stamp && ready.current;
    locked.current = true;
    setBusy(true);
    setError(null);
    try {
      const submitted = {
        ...draft,
        budget: {
          ...draft.budget,
          name: draft.budget.name.trim(),
          cloudProject: draft.budget.cloudProject.trim(),
          currency: draft.budget.currency.trim(),
          limitMicros: parseBudgetAmount(amount),
          targets: targets.split(",").map((t) => t.trim()),
        },
      };
      validateCloudBudget(submitted.budget);
      await saveBudgetDraft(client, AsyncStorage, key, submitted, alive);
      if (alive()) {
        install(null);
        await state.query.refetch();
      }
    } catch (e) {
      if (mounted.current && epoch.current.generation === stamp)
        setError(e instanceof Error ? e.message : String(e));
    } finally {
      locked.current = false;
      if (mounted.current) setBusy(false);
    }
  }
  function discard() {
    const stamp = epoch.current.generation;
    Alert.alert(
      "Discard GPU budget draft?",
      "The saved owner budget will remain unchanged.",
      [
        { text: "Keep draft", style: "cancel" },
        {
          text: "Discard draft",
          style: "destructive",
          onPress: () => {
            if (
              !ready.current ||
              locked.current ||
              epoch.current.generation !== stamp
            )
              return;
            locked.current = true;
            setBusy(true);
            void AsyncStorage.removeItem(key)
              .then(() => {
                if (mounted.current && epoch.current.generation === stamp) {
                  install(null);
                  setError(null);
                }
              })
              .catch((e) => {
                if (mounted.current && epoch.current.generation === stamp)
                  setError(String(e));
              })
              .finally(() => {
                locked.current = false;
                if (mounted.current) setBusy(false);
              });
          },
        },
      ],
    );
  }
  return (
    <KeyboardAvoidingView
      style={{ flex: 1, backgroundColor: theme.background }}
      behavior={Platform.OS === "ios" ? "padding" : undefined}
    >
      <FlatList
        data={state.data?.page.budgets ?? []}
        keyExtractor={(row) => row.budget.id}
        contentContainerStyle={[
          styles.content,
          { paddingTop: header + 16, paddingBottom: insets.bottom + 24 },
        ]}
        keyboardShouldPersistTaps="handled"
        ListHeaderComponent={
          <View style={styles.header}>
            <ConnectionBanner />
            <AutomationRefresh
              refreshing={state.query.isFetching}
              onRefresh={() => void state.query.refetch()}
            />
            <Text style={[styles.text, { color: theme.textSecondary }]}>
              Shared GPU allowances across runs. Held allowances remain reserved
              until provider billing is reconciled.
            </Text>
            {(error || state.query.isError) && (
              <Text
                accessibilityRole="alert"
                style={[styles.text, { color: theme.accent }]}
              >
                {error ?? String(state.query.error)}
              </Text>
            )}
            {!draft ? (
              <AutomationButton
                label="New GPU budget"
                disabled={disabled}
                onPress={() => start()}
              />
            ) : (
              <View style={styles.header}>
                <AutomationField
                  label="Name"
                  value={draft.budget.name}
                  editable={!disabled}
                  onChangeText={(name) => update({ name })}
                />
                <AutomationField
                  label="Nebius project ID"
                  value={draft.budget.cloudProject}
                  editable={!disabled}
                  autoCapitalize="none"
                  onChangeText={(cloudProject) => update({ cloudProject })}
                />
                <AutomationField
                  label="Currency"
                  value={draft.budget.currency}
                  editable={!disabled}
                  autoCapitalize="characters"
                  onChangeText={(currency) => update({ currency })}
                />
                <AutomationField
                  label="Total allowance"
                  value={amount}
                  editable={!disabled}
                  keyboardType="decimal-pad"
                  onChangeText={setAmount}
                />
                <AutomationField
                  label="Approved sandbox targets"
                  value={targets}
                  editable={!disabled}
                  autoCapitalize="none"
                  placeholder="Target names separated by commas"
                  onChangeText={setTargets}
                />
                <AutomationToggle
                  label="New GPU launches enabled"
                  value={!draft.budget.paused}
                  disabled={disabled}
                  onChange={(enabled) => update({ paused: !enabled })}
                />
                <AutomationButton
                  label={busy ? "Saving…" : "Save budget"}
                  disabled={disabled}
                  onPress={() => void save()}
                />
                <AutomationButton
                  label="Discard draft"
                  disabled={disabled}
                  onPress={discard}
                />
              </View>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.text, { color: theme.textSecondary }]}>
            No shared GPU budgets configured.
          </Text>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderColor: theme.border }]}>
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              {item.budget.name} · {item.budget.paused ? "Paused" : "Enabled"}
            </Text>
            <Text style={[styles.text, { color: theme.textSecondary }]}>
              {item.budget.cloudProject} · {item.budget.targets.join(", ")}
            </Text>
            <Text style={[styles.text, { color: theme.text }]}>
              {item.budget.currency} {formatBudgetAmount(item.heldMicros)} held
              · {formatBudgetAmount(item.availableMicros)} available ·{" "}
              {formatBudgetAmount(item.budget.limitMicros)} limit ·{" "}
              {item.requests} requests
            </Text>
            <AutomationButton
              label="Edit"
              disabled={disabled || draft !== null}
              onPress={() => start(item.budget)}
            />
          </View>
        )}
      />
    </KeyboardAvoidingView>
  );
}
const styles = StyleSheet.create({
  content: { paddingHorizontal: 20, gap: 16 },
  header: { gap: 12, paddingBottom: 16 },
  row: {
    gap: 8,
    paddingVertical: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  title: { fontSize: 18, fontWeight: "600", lineHeight: 26 },
  text: { fontSize: 15, lineHeight: 23 },
});
