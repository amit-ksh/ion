import { Stack, router, useLocalSearchParams } from "expo-router";
import { useEffect, useMemo, useState } from "react";
import { FlatList, RefreshControl, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { AutomationChoice } from "@/components/automation-form-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import { gatewayBillingRequest } from "@/lib/gateway-billing";
import type { GatewayBillingQuery } from "@waku/client";
type Row = {
  id: string;
  title: string;
  detail: string;
  amounts: string;
  provenance?: string;
  open?: () => void;
};
export default function GatewayCloudBillingScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    params = useLocalSearchParams<{
      owner?: string;
      scope?: string;
      export?: string;
    }>();
  const routeKey = JSON.stringify([params.owner, params.scope, params.export]);
  const [paging, setPaging] = useState<{
    identity: string;
    after?: string;
    previous: (string | undefined)[];
  }>({ identity: "", previous: [] });
  const route = useMemo(() => {
    try {
      return {
        ...gatewayBillingRequest({
          owner: params.owner,
          scope: params.scope,
          exportId: params.export,
          after: paging.identity.startsWith(routeKey + "|")
            ? paging.after
            : undefined,
        }),
        error: null,
      };
    } catch (cause) {
      return {
        query: {
          kind: "cloudBilling",
          query: { after: null, limit: 32 },
        } as GatewayBillingQuery,
        ownerId: undefined,
        error: cause instanceof Error ? cause.message : "Invalid billing route",
      };
    }
  }, [routeKey, paging]);
  const state = useGatewayPortfolio(route.query, {
    ownerId: route.ownerId,
    loadProjects: false,
    enabled: route.error === null,
  });
  const identity = `${routeKey}|${state.pairing?.baseUrl ?? ""}:${state.pairing?.id ?? ""}:${state.ownerId}`;
  const after = paging.identity === identity ? paging.after : undefined,
    previous = paging.identity === identity ? paging.previous : [];
  useEffect(() => {
    if (paging.identity !== identity) setPaging({ identity, previous: [] });
  }, [identity, paging.identity]);
  // Page transitions fence the previous request before its response can populate the view.
  const result =
    !route.error && (paging.identity === identity || paging.after === undefined)
      ? state.result
      : null;
  const page =
    result?.kind === "cloudBilling" ||
    result?.kind === "cloudBillingReports" ||
    result?.kind === "cloudBillingCharges"
      ? result.page
      : null;
  const rows = useMemo<Row[]>(
    () =>
      result?.kind === "cloudBilling"
        ? result.page.scopes.map((row) => ({
            id: row.id,
            title: row.name,
            detail: `${row.currency} · ${row.reports.length} recent monthly snapshots`,
            amounts: "",
            open: () =>
              router.push({
                pathname: "/gateway-cloud-billing",
                params: { owner: state.ownerId, scope: row.id },
              }),
          }))
        : result?.kind === "cloudBillingReports"
          ? result.page.reports.map((row) => ({
              id: row.exportId,
              title: `${row.period} · ${row.current ? "Selected monthly snapshot" : "Historical export"}`,
              detail: `${row.exportId} · Created ${row.createdAt}`,
              amounts: `${row.currency} ${row.billedCost} billed · ${row.effectiveCost} effective · ${row.contractedCost} contracted`,
              provenance: `CSV SHA-256 ${row.csvSha256} · Archive SHA-256 ${row.archiveSha256}`,
              open: () =>
                router.push({
                  pathname: "/gateway-cloud-billing",
                  params: {
                    owner: state.ownerId,
                    scope: row.scopeId,
                    export: row.exportId,
                  },
                }),
            }))
          : result?.kind === "cloudBillingCharges"
            ? result.page.charges.map((row) => ({
                id: String(row.ordinal),
                title: `${row.category} · ${row.class ?? "Original charge"}`,
                detail: `${row.resourceType ?? "Unspecified resource type"} · ${row.chargePeriodStart} – ${row.chargePeriodEnd}`,
                amounts: `${result.page.report.currency} ${row.billedCost} billed · ${row.effectiveCost} effective · ${row.contractedCost} contracted`,
              }))
            : [],
    [result, state.ownerId],
  );
  const title = params.export
      ? "Billing charges"
      : params.scope
        ? "Imported exports"
        : "Cloud billing",
    next = page?.nextCursor;
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <Stack.Screen options={{ title }} />
      <FlatList
        data={rows}
        keyExtractor={(row) => row.id}
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
              {title}
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
              disabled={!!params.owner || !!route.error || !state.owners.length}
              onChange={(value) => {
                setPaging({ identity: "", previous: [] });
                state.selectOwner(value);
              }}
            />
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              Account charges from saved Nebius reports. Import reports on the
              desktop.
            </Text>
            {result && (
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                Updated {new Date(result.servedAt).toLocaleString()}
                {result.pageTruncated ? " · More records on the next page" : ""}
              </Text>
            )}
            {result?.kind === "cloudBillingCharges" && (
              <>
                <Text selectable style={[styles.body, { color: theme.text }]}>
                  {result.page.report.period} · {result.page.report.currency}{" "}
                  {result.page.report.billedCost} billed ·{" "}
                  {result.page.report.current
                    ? "Selected monthly snapshot"
                    : "Historical export"}
                </Text>
                <Text
                  selectable
                  style={[styles.body, { color: theme.textSecondary }]}
                >
                  {result.page.report.exportId} · Created{" "}
                  {result.page.report.createdAt} ·{" "}
                  {result.page.report.unattributedResourceRows} rows without
                  resource attribution
                </Text>
                <Text
                  selectable
                  style={[styles.body, { color: theme.textSecondary }]}
                >
                  CSV SHA-256 {result.page.report.csvSha256} · Archive SHA-256{" "}
                  {result.page.report.archiveSha256}
                </Text>
              </>
            )}
            {(route.error || state.message) && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.text }]}
              >
                {route.error ?? state.message}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {route.error || state.message
              ? ""
              : state.loading
                ? "Loading billing…"
                : result
                  ? params.scope
                    ? "No imported billing records in this page."
                    : "Create a billing connection and import a report on your desktop."
                  : "Connect and refresh to read cloud billing."}
          </Text>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text
              accessibilityRole="header"
              style={[styles.heading, { color: theme.text }]}
            >
              {item.title}
            </Text>
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              {item.detail}
            </Text>
            {!!item.amounts && (
              <Text selectable style={[styles.body, { color: theme.text }]}>
                {item.amounts}
              </Text>
            )}
            {item.provenance && (
              <Text
                selectable
                style={[styles.body, { color: theme.textSecondary }]}
              >
                {item.provenance}
              </Text>
            )}
            {item.open && (
              <AutomationButton
                label={params.scope ? "View charges" : "Imported exports"}
                disabled={state.loading}
                onPress={item.open}
              />
            )}
          </View>
        )}
        ListFooterComponent={
          <View style={styles.footer}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous page"
                disabled={state.loading || !result}
                onPress={() =>
                  setPaging({
                    identity,
                    after: previous.at(-1),
                    previous: previous.slice(0, -1),
                  })
                }
              />
            )}
            {next !== null && next !== undefined && (
              <AutomationButton
                label="Next page"
                disabled={state.loading || !result}
                onPress={() =>
                  setPaging({
                    identity,
                    after: String(next),
                    previous: [...previous, after],
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
