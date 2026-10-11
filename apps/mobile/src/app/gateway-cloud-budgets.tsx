import {
  formatBudgetAmount,
  portfolioId,
  type PortfolioQuery,
} from "@waku/client";
import { useLocalSearchParams } from "expo-router";
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
export default function GatewayCloudBudgetsScreen() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    params = useLocalSearchParams<{ owner?: string }>();
  const [after, setAfter] = useState<string | null>(null),
    [previous, setPrevious] = useState<(string | null)[]>([]);
  const query = useMemo<PortfolioQuery>(
    () => ({ kind: "cloudBudgets", query: { after, limit: 32 } }),
    [after],
  );
  const state = useGatewayPortfolio(query, {
    ownerId:
      typeof params.owner === "string" && portfolioId(params.owner)
        ? params.owner
        : undefined,
    loadProjects: false,
  });
  const page = state.result?.kind === "cloudBudgets" ? state.result.page : null;
  const identity = state.pairing
    ? `${state.pairing.baseUrl}:${state.pairing.id}:${state.ownerId}`
    : "";
  useEffect(() => {
    setAfter(null);
    setPrevious([]);
  }, [identity]);
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <FlatList
        data={page?.budgets ?? []}
        keyExtractor={(row) => row.id}
        contentContainerStyle={[
          styles.content,
          { paddingBottom: insets.bottom + 24 },
        ]}
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
            <Text style={[styles.text, { color: theme.textSecondary }]}>
              Shared GPU allowances across runs. Held allowances remain reserved
              until provider billing is reconciled.
            </Text>
            {state.message && (
              <Text
                accessibilityRole="alert"
                style={[styles.text, { color: theme.text }]}
              >
                {state.message}
              </Text>
            )}
            {state.result && (
              <Text style={[styles.text, { color: theme.textSecondary }]}>
                Updated {new Date(state.result.servedAt).toLocaleString()}
              </Text>
            )}
            {page && (
              <Text style={[styles.text, { color: theme.textSecondary }]}>
                {page.total} GPU budgets
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.text, { color: theme.textSecondary }]}>
            {state.loading
              ? "Loading GPU budgets…"
              : "No GPU budgets in this page."}
          </Text>
        }
        renderItem={({ item }) => (
          <View style={[styles.row, { borderColor: theme.border }]}>
            <Text
              accessibilityRole="header"
              style={[styles.title, { color: theme.text }]}
            >
              {item.name} · {item.paused ? "Paused" : "Enabled"}
            </Text>
            <Text style={[styles.text, { color: theme.text }]}>
              {item.currency} {formatBudgetAmount(item.heldMicros)} held ·{" "}
              {formatBudgetAmount(item.availableMicros)} available ·{" "}
              {formatBudgetAmount(item.limitMicros)} limit · {item.requests}{" "}
              requests
            </Text>
          </View>
        )}
        ListFooterComponent={
          <View style={styles.header}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous page"
                disabled={state.loading}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {page?.nextCursor && (
              <AutomationButton
                label="Next page"
                disabled={state.loading}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(page.nextCursor);
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
  content: { padding: 20, gap: 16 },
  header: { gap: 12, paddingVertical: 16 },
  row: {
    gap: 8,
    paddingVertical: 16,
    borderBottomWidth: StyleSheet.hairlineWidth,
  },
  title: { fontSize: 18, fontWeight: "600", lineHeight: 26 },
  text: { fontSize: 15, lineHeight: 23 },
});
