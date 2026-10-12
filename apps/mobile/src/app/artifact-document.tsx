import type { ArtifactLibraryItem } from "@waku/client";
import * as Crypto from "expo-crypto";
import { Stack, useFocusEffect, useLocalSearchParams } from "expo-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { AppState, FlatList, StyleSheet, Text, View } from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { ConnectionBanner } from "@/components/connection-banner";
import { useScreenHeaderInset } from "@/components/screen-header";
import { useMarkdownStyles } from "@/components/md-block-row";
import { useTheme } from "@/hooks/use-theme";
import {
  artifactRoute,
  artifactText,
  readArtifactDocument,
} from "@/lib/artifact-library";
import { documentPreviewRows } from "@/lib/document-preview";
import { useDaemon } from "@/lib/daemon-context";
import { exportWorkflowDocument } from "@/lib/workflow-document-export";
import { renderBlock } from "@/md/render";
export default function ArtifactDocumentScreen() {
  const params = useLocalSearchParams<{
      owner?: string;
      document?: string;
      address?: string;
    }>(),
    daemon = useDaemon(),
    theme = useTheme(),
    insets = useSafeAreaInsets(),
    header = useScreenHeaderInset();
  const route = useMemo(() => {
    try {
      return { value: artifactRoute(params), error: null };
    } catch (error) {
      return {
        value: null,
        error:
          error instanceof Error
            ? error.message
            : "Open a document from Artifacts.",
      };
    }
  }, [params.owner, params.document, params.address]);
  const markdown = useMarkdownStyles(),
    markdownStyles = useMemo(
      () => ({
        ...markdown,
        body: { ...markdown.body, fontSize: 16, lineHeight: 24 },
      }),
      [markdown],
    );
  const [focused, setFocused] = useState(false),
    [foreground, setForeground] = useState(AppState.currentState === "active"),
    [refresh, setRefresh] = useState(0),
    [reading, setReading] = useState(true),
    [busy, setBusy] = useState(false),
    [exporting, setExporting] = useState(false);
  const [loaded, setLoaded] = useState<{
      scope: string;
      document: ArtifactLibraryItem;
      bytes: Uint8Array;
      text: string | null;
    } | null>(null),
    [message, setMessage] = useState<{
      scope: string;
      text: string;
      error: boolean;
    } | null>(null);
  const generation = useRef(0),
    visible = useRef(false),
    mounted = useRef(true),
    locked = useRef(false),
    daemonRef = useRef(daemon);
  daemonRef.current = daemon;
  const scope = JSON.stringify([
    params.owner,
    params.document,
    params.address,
    daemon.activeProfile?.id,
    daemon.activeProfile?.address,
    daemon.connections,
    daemon.phase,
    daemon.client?.clientId,
    refresh,
  ]);
  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current++;
    };
  }, []);
  useFocusEffect(
    useCallback(() => {
      visible.current = true;
      setFocused(true);
      generation.current++;
      return () => {
        visible.current = false;
        setFocused(false);
        generation.current++;
      };
    }, []),
  );
  useEffect(() => {
    const subscription = AppState.addEventListener("change", (state) => {
      generation.current++;
      setForeground(state === "active");
      setLoaded(null);
      setMessage(null);
    });
    return () => subscription.remove();
  }, []);
  useEffect(() => {
    const ticket = ++generation.current;
    setLoaded(null);
    setMessage(null);
    setReading(true);
    const client = daemon.client,
      selected = route.value,
      started = scope;
    if (
      !focused ||
      !foreground ||
      !client ||
      !selected ||
      daemon.phase !== "connected" ||
      daemon.activeProfile?.id !== selected.owner ||
      daemon.activeProfile.address !== selected.address
    ) {
      setBusy(false);
      return;
    }
    const current = () =>
      mounted.current &&
      visible.current &&
      AppState.currentState === "active" &&
      generation.current === ticket &&
      scopeRef.current === started &&
      daemonRef.current.client === client &&
      daemonRef.current.phase === "connected" &&
      daemonRef.current.activeProfile?.id === selected.owner &&
      daemonRef.current.activeProfile.address === selected.address;
    setBusy(true);
    void readArtifactDocument(
      client,
      selected.document,
      current,
      async (bytes) => {
        const digest = new Uint8Array(
          await Crypto.digest(
            Crypto.CryptoDigestAlgorithm.SHA256,
            bytes.buffer as ArrayBuffer,
          ),
        );
        return [...digest]
          .map((byte) => byte.toString(16).padStart(2, "0"))
          .join("");
      },
    )
      .then((bytes) => {
        if (!current()) return;
        let text: string | null = null;
        try {
          text = artifactText(selected.document, bytes);
        } catch {
          /* Invalid textual bytes remain exportable as exact original content. */
        }
        setLoaded({ scope: started, document: selected.document, bytes, text });
      })
      .catch((error) => {
        if (current())
          setMessage({
            scope: started,
            text:
              error instanceof Error
                ? error.message
                : "Document unavailable. Refresh the artifact library.",
            error: true,
          });
      })
      .finally(() => {
        if (current()) setBusy(false);
      });
    return () => {
      generation.current++;
    };
  }, [scope, focused, foreground, daemon.client, route.value]);
  const preview =
    focused &&
    foreground &&
    daemon.phase === "connected" &&
    route.value?.owner === daemon.activeProfile?.id &&
    route.value?.address === daemon.activeProfile?.address &&
    loaded?.scope === scope
      ? loaded
      : null;
  const rows = useMemo(
    () =>
      preview?.text !== null && preview?.text !== undefined
        ? documentPreviewRows(preview.document.format, preview.text, reading)
        : [],
    [preview, reading],
  );
  async function save() {
    if (
      !preview ||
      locked.current ||
      !visible.current ||
      AppState.currentState !== "active"
    )
      return;
    const started = scope,
      ticket = generation.current,
      client = daemon.client;
    const current = () =>
      mounted.current &&
      visible.current &&
      AppState.currentState === "active" &&
      scopeRef.current === started &&
      generation.current === ticket &&
      daemonRef.current.client === client &&
      daemonRef.current.phase === "connected";
    locked.current = true;
    setExporting(true);
    setMessage(null);
    try {
      await exportWorkflowDocument(preview.document, preview.bytes, current);
      if (current())
        setMessage({
          scope: started,
          text: "Verified document exported.",
          error: false,
        });
    } catch (error) {
      if (current())
        setMessage({
          scope: started,
          text:
            error instanceof Error
              ? error.message
              : "Export failed. Choose a folder and retry.",
          error: true,
        });
    } finally {
      locked.current = false;
      if (mounted.current) setExporting(false);
    }
  }
  const selected = route.value?.document;
  return (
    <View style={{ flex: 1, backgroundColor: theme.background }}>
      <Stack.Screen options={{ title: selected?.name ?? "Document" }} />
      <FlatList
        data={rows}
        keyExtractor={(row) => row.id}
        initialNumToRender={12}
        maxToRenderPerBatch={12}
        contentContainerStyle={{
          paddingTop: header + 8,
          paddingBottom: insets.bottom + 24,
        }}
        ListHeaderComponent={
          <View style={styles.header}>
            <ConnectionBanner />
            <AutomationRefresh
              refreshing={busy || exporting || daemon.phase !== "connected"}
              onRefresh={() => setRefresh((value) => value + 1)}
            />
            {selected && (
              <>
                <Text
                  accessibilityRole="header"
                  style={[styles.title, { color: theme.text }]}
                >
                  {selected.name}
                </Text>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {selected.format.toUpperCase()} ·{" "}
                  {selected.bytes.toLocaleString()} bytes
                </Text>
              </>
            )}
            {route.error && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.danger }]}
              >
                {route.error}
              </Text>
            )}
            {route.value &&
              (route.value.owner !== daemon.activeProfile?.id ||
                route.value.address !== daemon.activeProfile?.address) && (
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  Connect to the daemon where this document was published, or
                  reopen it from Artifacts.
                </Text>
              )}
            {preview && (
              <>
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {preview.text === null
                    ? "Verified original document · Export available"
                    : reading &&
                        ["md", "json"].includes(preview.document.format)
                      ? "Verified document · Reading view"
                      : "Verified document · Source"}
                </Text>
                <View style={styles.actions}>
                  {preview.text !== null &&
                    ["md", "json"].includes(preview.document.format) && (
                      <>
                        <AutomationButton
                          label="Reading view"
                          disabled={reading || exporting}
                          onPress={() => setReading(true)}
                        />
                        <AutomationButton
                          label="Source"
                          disabled={!reading || exporting}
                          onPress={() => setReading(false)}
                        />
                      </>
                    )}
                  <AutomationButton
                    label="Export document"
                    disabled={exporting}
                    onPress={() => void save()}
                  />
                </View>
                {preview.text === null && (
                  <Text style={[styles.body, { color: theme.textSecondary }]}>
                    {["pdf", "docx"].includes(preview.document.format)
                      ? "Export the verified original to read it in your document app."
                      : "This document has no supported text preview. Export the verified original."}
                  </Text>
                )}
                {["html", "htm"].includes(preview.document.format) &&
                  preview.text !== null && (
                    <Text style={[styles.body, { color: theme.textSecondary }]}>
                      HTML source · Scripts, links and images stay inactive.
                    </Text>
                  )}
              </>
            )}
            {message?.scope === scope && (
              <Text
                accessibilityRole={message.error ? "alert" : undefined}
                style={[
                  styles.body,
                  { color: message.error ? theme.danger : theme.textSecondary },
                ]}
              >
                {message.text}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          busy ? (
            <Text style={[styles.empty, { color: theme.textSecondary }]}>
              Verifying complete document…
            </Text>
          ) : null
        }
        renderItem={({ item }) =>
          item.node ? (
            <View style={{ paddingHorizontal: 20, paddingBottom: 14 }}>
              {renderBlock(
                item.node,
                {
                  styles: markdownStyles,
                  veil: null,
                  now: 0,
                  ordinal: { value: 0 },
                  hadSpans: false,
                  onOpenLink: () => {},
                },
                item.id,
              )}
            </View>
          ) : (
            <Text selectable style={[styles.source, { color: theme.text }]}>
              {item.text}
            </Text>
          )
        }
      />
    </View>
  );
}
const styles = StyleSheet.create({
  header: { paddingHorizontal: 20, gap: 12, paddingBottom: 20 },
  title: { fontSize: 20, fontWeight: "600", lineHeight: 28 },
  body: { fontSize: 14, lineHeight: 21 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  source: {
    fontSize: 15,
    lineHeight: 23,
    paddingHorizontal: 20,
    paddingBottom: 12,
  },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
});
