import * as Clipboard from "expo-clipboard";
import { useCallback, useEffect, useRef, useState } from "react";
import { Stack, useFocusEffect } from "expo-router";
import { Alert, FlatList, Linking, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { useTheme } from "@/hooks/use-theme";
import { useScreenHeaderInset } from "@/components/screen-header";
import { cloudAccount } from "@/lib/cloud-account";
import {
  hostingSites,
  hostingAction,
  removeHostedVersion,
  type HostedSite,
} from "@/lib/cloud-hosting";
export default function CloudDeployments() {
  const theme = useTheme(),
    insets = useSafeAreaInsets(),
    header = useScreenHeaderInset();
  const [items, setItems] = useState<HostedSite[]>([]),
    [error, setError] = useState<string | null>(null),
    [busy, setBusy] = useState(false);
  const generation = useRef(0),
    active = useRef(false),
    running = useRef(false);
  async function reload() {
    if (running.current) return;
    running.current = true;
    const run = ++generation.current;
    setBusy(true);
    setError(null);
    try {
      const items = await hostingSites();
      if (active.current && run === generation.current) setItems(items);
    } catch (error) {
      if (active.current && run === generation.current)
        setError(
          error instanceof Error ? error.message : "Deployments unavailable.",
        );
    } finally {
      running.current = false;
      if (active.current && run === generation.current) setBusy(false);
    }
  }
  useFocusEffect(
    useCallback(() => {
      active.current = true;
      void reload();
      return () => {
        active.current = false;
        generation.current++;
      };
    }, []),
  );
  useEffect(
    () =>
      cloudAccount.onSignOut(() => {
        generation.current++;
        setItems([]);
        setError("Sign in to Ion Cloud to view deployments.");
      }),
    [],
  );
  async function action(
    site: HostedSite,
    kind: "view" | "stop" | "start" | "visibility" | "rollback" | "logs",
    body: unknown = {},
  ) {
    if (running.current) return;
    running.current = true;
    setBusy(true);
    setError(null);
    const run = ++generation.current;
    try {
      const result = await hostingAction(site, kind, body);
      if (!active.current || run !== generation.current) return;
      if (kind === "view") {
        const url = new URL(result.url);
        if (
          url.protocol !== "https:" ||
          url.hostname !== new URL(site.url).hostname
        )
          throw new Error("Invalid deployment link");
        await Linking.openURL(url.toString());
      }
      setItems(await hostingSites());
    } catch (error) {
      if (active.current && run === generation.current)
        setError(
          error instanceof Error ? error.message : "Deployment update failed.",
        );
    } finally {
      running.current = false;
      if (active.current && run === generation.current) setBusy(false);
    }
  }
  return (
    <>
      <Stack.Screen options={{ title: "Deployments" }} />
      <FlatList
        data={items}
        keyExtractor={(item) => item.id}
        contentContainerStyle={{
          paddingHorizontal: 20,
          paddingTop: header + 12,
          paddingBottom: insets.bottom + 24,
          gap: 24,
        }}
        ListHeaderComponent={
          <View style={{ gap: 12 }}>
            <AutomationRefresh
              refreshing={busy}
              onRefresh={() => void reload()}
            />
            <Text style={{ color: theme.textSecondary }}>
              Publish HTML from a verified artifact. Deploy project source from
              the desktop app.
            </Text>
            {error && (
              <Text accessibilityRole="alert" style={{ color: theme.danger }}>
                {error}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          !busy ? (
            <Text style={{ color: theme.textSecondary }}>
              No deployments yet.
            </Text>
          ) : null
        }
        renderItem={({ item }) => (
          <View
            style={{
              gap: 12,
              borderTopWidth: 1,
              borderTopColor: theme.border,
              paddingTop: 16,
            }}
          >
            <Text
              accessibilityRole="header"
              style={{ fontSize: 18, fontWeight: "600", color: theme.text }}
            >
              {item.title}
            </Text>
            <Text selectable style={{ color: theme.textSecondary }}>
              {item.url}
            </Text>
            <Text style={{ color: theme.textSecondary }}>
              {item.stopped
                ? "Stopped"
                : item.active
                  ? "Active"
                  : "Awaiting first version"}{" "}
              · {item.visibility}
            </Text>
            <View style={{ flexDirection: "row", flexWrap: "wrap", gap: 8 }}>
              <AutomationButton
                label="Copy link"
                onPress={() => void Clipboard.setStringAsync(item.url)}
              />
              <AutomationButton
                label="Open"
                disabled={busy || !item.active || item.stopped}
                onPress={() => void action(item, "view")}
              />
              <AutomationButton
                label="Runtime logs"
                disabled={busy || !item.active}
                onPress={() => void action(item, "logs")}
              />
              <AutomationButton
                label={item.stopped ? "Start" : "Stop / Unpublish"}
                disabled={busy}
                onPress={() =>
                  void action(item, item.stopped ? "start" : "stop")
                }
              />
              <AutomationButton
                label={
                  item.visibility === "public" ? "Make private" : "Make public"
                }
                disabled={busy}
                onPress={() =>
                  Alert.alert(
                    "Change visibility",
                    item.visibility === "public"
                      ? "Restrict access to your account?"
                      : "Anyone with the link will be able to open this deployment.",
                    [
                      { text: "Cancel", style: "cancel" },
                      {
                        text: "Confirm",
                        onPress: () =>
                          void action(item, "visibility", {
                            visibility:
                              item.visibility === "public"
                                ? "private"
                                : "public",
                          }),
                      },
                    ],
                  )
                }
              />
            </View>
            {item.releases.map((release) => (
              <View key={release.id} style={{ gap: 8 }}>
                <Text style={{ color: theme.textSecondary }}>
                  {release.id.slice(0, 8)} · {release.status}
                </Text>
                <Text style={{ color: theme.textSecondary }}>
                  {release.logs}
                </Text>
                {item.active !== release.id &&
                  ["ready", "failed", "uncertain"].includes(release.status) && (
                    <AutomationButton
                      label="Remove inactive version"
                      disabled={busy}
                      destructive
                      onPress={() =>
                        Alert.alert(
                          "Remove inactive version?",
                          "This removes its retained source and runtime after the worker reconnects.",
                          [
                            { text: "Cancel", style: "cancel" },
                            {
                              text: "Remove",
                              style: "destructive",
                              onPress: () => {
                                void removeHostedVersion(item, release)
                                  .then(() => reload())
                                  .catch((error) => setError(String(error)));
                              },
                            },
                          ],
                        )
                      }
                    />
                  )}
                {release.status === "ready" && item.active !== release.id && (
                  <AutomationButton
                    label="Roll back to this version"
                    disabled={busy}
                    onPress={() =>
                      void action(item, "rollback", { releaseId: release.id })
                    }
                  />
                )}
              </View>
            ))}
          </View>
        )}
      />
    </>
  );
}
