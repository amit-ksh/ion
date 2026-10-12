import { Stack, router, useLocalSearchParams } from "expo-router";
import { useCallback, useEffect, useState } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { useTheme } from "@/hooks/use-theme";
import { useAutomationQuery } from "@/hooks/use-automation-query";
import { readCloudBilling } from "@/lib/cloud-billing";
import type { WakuClient } from "@waku/client";
export default function CloudBillingScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    params = useLocalSearchParams<{ scope?: string; export?: string }>();
  const scope = typeof params.scope === "string" ? params.scope : undefined,
    exportId = typeof params.export === "string" ? params.export : undefined;
  const [after, setAfter] = useState<string | undefined>(),
    [previous, setPrevious] = useState<(string | undefined)[]>([]);
  const load = useCallback(
    (client: WakuClient) =>
      readCloudBilling(client, { scope, exportId, after }),
    [scope, exportId, after],
  );
  const state = useAutomationQuery(
    ["cloud-billing", scope, exportId, after],
    load,
    { poll: false },
  );
  useEffect(() => {
    setAfter(undefined);
    setPrevious([]);
  }, [
    scope,
    exportId,
    state.daemon.activeProfile?.id,
    state.daemon.activeProfile?.address,
  ]);
  const data = state.fresh && !state.query.isFetching ? state.data : undefined;
  const rows: {
    id: string;
    title: string;
    detail: string;
    meter: string;
    open: (() => void) | null;
  }[] =
    data?.kind === "accounts"
      ? data.page.scopes.map((row) => ({
          id: row.scope.id,
          title: row.scope.name,
          detail: `${row.scope.currency} · ${row.reports.length} recent monthly snapshots`,
          meter: "",
          open: () =>
            router.push({
              pathname: "/cloud-billing",
              params: { scope: row.scope.id },
            }),
        }))
      : data?.kind === "reports"
        ? data.page.reports.map((row) => ({
            id: row.exportId,
            title: `${row.period} · ${row.current ? "Selected monthly snapshot" : "Historical export"}`,
            detail: `${row.exportId} · Created ${row.createdAt}`,
            meter: `${row.currency} ${row.billedCost} billed · ${row.effectiveCost} effective · ${row.contractedCost} contracted`,
            open: () =>
              router.push({
                pathname: "/cloud-billing",
                params: { scope: row.scopeId, export: row.exportId },
              }),
          }))
        : data?.kind === "charges"
          ? data.page.charges.map((row) => ({
              id: String(row.ordinal),
              title: `${row.category} · ${row.class ?? "Original charge"}`,
              detail: `${row.containerId} · ${row.resourceType ?? "Unspecified resource type"} · ${row.resourceId ?? "Resource attribution unavailable"}`,
              meter: `Billed ${row.billedCost} · Effective ${row.effectiveCost} · Contracted ${row.contractedCost}`,
              open: null,
            }))
          : [];
  const cursor = data?.page.nextCursor;
  const next =
    cursor === null || cursor === undefined ? undefined : String(cursor);
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <Stack.Screen
        options={{
          title: exportId
            ? "Billing charges"
            : scope
              ? "Imported exports"
              : "Cloud billing",
        }}
      />
      <FlatList
        data={rows}
        keyExtractor={(row) => row.id}
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
              {exportId
                ? "Billing charges"
                : scope
                  ? "Imported exports"
                  : "Cloud billing"}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              Account charges from saved Nebius reports.
            </Text>
            {data?.kind === "charges" && (
              <Text style={[styles.body, { color: theme.text }]}>
                {data.page.report.period} · {data.page.report.currency}{" "}
                {data.page.report.billedCost} billed ·{" "}
                {data.page.report.current
                  ? "Selected monthly snapshot"
                  : "Historical export"}
              </Text>
            )}
            <AutomationRefresh
              refreshing={state.query.isFetching}
              onRefresh={() => {
                if (state.visible && state.daemon.phase === "connected")
                  void state.query.refetch();
              }}
            />
            {state.query.isError && (
              <Text
                accessibilityLiveRegion="polite"
                style={[styles.body, { color: theme.text }]}
              >
                {state.query.error instanceof Error
                  ? state.query.error.message
                  : "Could not read billing. Refresh to retry."}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.query.isFetching
              ? "Loading billing…"
              : !state.fresh
                ? "Connect and refresh to read cloud billing."
                : scope
                  ? "No imported billing records."
                  : "Create a billing connection and sync a report on your desktop."}
          </Text>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.heading, { color: theme.text }]}>
              {item.title}
            </Text>
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              {item.detail}
            </Text>
            {item.meter && (
              <Text selectable style={[styles.body, { color: theme.text }]}>
                {item.meter}
              </Text>
            )}
            {item.open && (
              <AutomationButton
                label={scope ? "View charges" : "Imported exports"}
                onPress={item.open}
              />
            )}
          </View>
        )}
        ListFooterComponent={
          <View style={styles.footer}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous"
                disabled={!data}
                onPress={() => {
                  const prior = previous.at(-1);
                  setPrevious(previous.slice(0, -1));
                  setAfter(prior);
                }}
              />
            )}
            {next && (
              <AutomationButton
                label="Next"
                disabled={!data}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(next);
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
  title: { fontSize: 22, lineHeight: 30, fontWeight: "600" },
  body: { fontSize: 16, lineHeight: 24 },
  heading: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  row: { padding: 20, gap: 10, borderBottomWidth: 1 },
  empty: { padding: 20, fontSize: 16, lineHeight: 24 },
  footer: { padding: 20, gap: 10 },
});
