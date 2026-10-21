import { useEffect, useRef, useState } from "react";
import { Alert, Switch, Text, View } from "react-native";
import { router } from "expo-router";
import type { ArtifactLibraryItem, WakuClient } from "@waku/client";
import { AutomationButton } from "./automation-controls";
import { useTheme } from "@/hooks/use-theme";
import { cloudAccount } from "@/lib/cloud-account";
import { prepareHtmlPublication } from "@/lib/cloud-hosting";
import { MOBILE_CLOUD_SERVICES } from '@/lib/mobile-capabilities';
export function CloudArtifactPublish(props: Parameters<typeof EnabledCloudArtifactPublish>[0]) {
  return MOBILE_CLOUD_SERVICES ? <EnabledCloudArtifactPublish {...props} /> : null;
}
function EnabledCloudArtifactPublish({
  client,
  document,
  bytes,
  current,
}: {
  client: WakuClient;
  document: ArtifactLibraryItem;
  bytes: Uint8Array;
  current: () => boolean;
}) {
  const theme = useTheme(),
    [busy, setBusy] = useState(false),
    [interactive, setInteractive] = useState(false),
    [error, setError] = useState<string | null>(null),
    alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  if (document.format !== "html") return null;
  async function review() {
    if (busy || !current()) return;
    setBusy(true);
    setError(null);
    try {
      const account = await cloudAccount.status();
      if (!account.signedIn)
        throw new Error("Sign in to Ion Cloud from Daemons first.");
      const valid = () => alive.current && current();
      const input = await prepareHtmlPublication(
        client,
        document,
        bytes,
        valid,
      );
      input.settings.runtime = interactive ? "static" : "document";
      if (!valid()) return;
      Alert.alert(
        "Publish HTML",
        "Upload " +
          input.files.length +
          " verified files to Ion Cloud. Choose who can open it. Script access follows the selected interactive HTML setting.",
        [
          { text: "Cancel", style: "cancel" },
          {
            text: "Publish privately",
            onPress: () => void publish(input, "private", account.workspaceId!),
          },
          {
            text: "Publish publicly",
            onPress: () => void publish(input, "public", account.workspaceId!),
          },
        ],
      );
    } catch (error) {
      if (alive.current)
        setError(
          error instanceof Error
            ? error.message
            : "Could not prepare publication.",
        );
    } finally {
      if (alive.current) setBusy(false);
    }
  }
  async function publish(
    input: Awaited<ReturnType<typeof prepareHtmlPublication>>,
    visibility: "private" | "public",
    workspace: string,
  ) {
    if (!alive.current || !current()) return;
    setBusy(true);
    setError(null);
    try {
      if ((await cloudAccount.status()).workspaceId !== workspace)
        throw new Error("Account changed; review this publication again.");
      await cloudAccount.request("/v1/hosting/releases", "POST", {
        ...input,
        visibility,
      });
      if (alive.current && current()) router.push("/cloud-deployments");
    } catch (error) {
      if (alive.current)
        setError(
          error instanceof Error
            ? error.message
            : "Publishing failed. Check Deployments before trying again.",
        );
    } finally {
      if (alive.current) setBusy(false);
    }
  }
  return (
    <View style={{ gap: 8 }}>
      <View style={{ flexDirection: "row", alignItems: "center", gap: 12 }}>
        <Switch
          accessibilityLabel="Allow scripts on isolated website"
          disabled={busy}
          value={interactive}
          onValueChange={setInteractive}
        />
        <Text style={{ color: theme.textSecondary, flex: 1 }}>
          Allow interactive HTML on an isolated website
        </Text>
      </View>
      <AutomationButton
        label={busy ? "Preparing publication…" : "Publish HTML"}
        disabled={busy}
        onPress={() => void review()}
      />
      {error && (
        <Text accessibilityRole="alert" style={{ color: theme.danger }}>
          {error}
        </Text>
      )}
    </View>
  );
}
