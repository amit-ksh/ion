import { PdfPageViewer, pdfDigest } from "@/components/artifact-pdf-preview";
import { readGatewayPdfPage } from "@/lib/gateway-pdf";
import {
  documentPreviewRows,
  prepareDocumentReading,
  type DocumentPreviewRow,
  type DocumentReadingPreview,
} from "@/lib/document-preview";
import { useMarkdownStyles } from "@/components/md-block-row";
import { renderBlock } from "@/md/render";
import {
  portfolioId,
  type PortfolioQuery,
  type WorkflowDocument,
} from "@waku/client";
import * as Crypto from "expo-crypto";
import { Stack, useFocusEffect, useLocalSearchParams } from "expo-router";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AppState,
  FlatList,
  RefreshControl,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { useSafeAreaInsets } from "react-native-safe-area-context";
import {
  AutomationButton,
  AutomationRefresh,
} from "@/components/automation-controls";
import { useGatewayPortfolio } from "@/hooks/use-gateway-portfolio";
import { useTheme } from "@/hooks/use-theme";
import {
  readGatewayWorkflowDocument,
  workflowDocumentText,
} from "@/lib/gateway-workflow-document";
import { exportWorkflowDocument } from "@/lib/workflow-document-export";
export default function GatewayWorkflowEvidenceScreen() {
  const params = useLocalSearchParams<{ run: string; owner: string }>(),
    theme = useTheme(),
    insets = useSafeAreaInsets();
  const markdown = useMarkdownStyles();
  const [pdfPreview, setPdfPreview] = useState<{
    document: WorkflowDocument;
    revision: number;
    scope: string;
    ticket: number;
  } | null>(null);
  const [readingView, setReadingView] = useState(true);
  const markdownStyles = useMemo(
    () => ({
      ...markdown,
      body: { ...markdown.body, fontSize: 16, lineHeight: 24 },
    }),
    [markdown],
  );
  const [after, setAfter] = useState<string | null>(null),
    [previous, setPrevious] = useState<(string | null)[]>([]);
  const [reading, setReading] = useState(false),
    [message, setMessage] = useState<{ text: string; error: boolean } | null>(
      null,
    ),
    [preview, setPreview] = useState<{
      name: string;
      text: string;
      format: string;
      sourceRows: DocumentPreviewRow[];
      readingPreview: DocumentReadingPreview;
      scope: string;
    } | null>(null);
  const valid =
    typeof params.run === "string" &&
    portfolioId(params.run) &&
    typeof params.owner === "string" &&
    portfolioId(params.owner);
  const query = useMemo<PortfolioQuery>(
    () => ({
      kind: "workflowEvidence",
      query: {
        runId: typeof params.run === "string" ? params.run : "",
        after,
        limit: 32,
      },
    }),
    [params.run, after],
  );
  const state = useGatewayPortfolio(query, {
    ownerId: typeof params.owner === "string" ? params.owner : "",
    loadProjects: false,
  });
  const page =
    valid && state.result?.kind === "workflowEvidence"
      ? state.result.page
      : null;
  const scope = JSON.stringify([
    params.run,
    params.owner,
    state.pairing?.baseUrl,
    state.pairing?.id,
    state.pairing?.token,
    state.pairing?.expiresAt,
    state.pairing?.grants,
    state.ownerId,
    after,
  ]);
  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  const visible = useRef(false),
    epoch = useRef(0),
    locked = useRef(false),
    mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  useEffect(() => {
    epoch.current++;
    setPreview(null);
    setPdfPreview(null);
    setMessage(null);
  }, [scope]);
  useEffect(() => {
    setAfter(null);
    setPrevious([]);
  }, [params.run, params.owner, state.pairing?.id, state.pairing?.baseUrl]);
  useFocusEffect(
    useCallback(() => {
      visible.current = true;
      epoch.current++;
      setPreview(null);
      setPdfPreview(null);
      const subscription = AppState.addEventListener("change", () => {
        epoch.current++;
        setPreview(null);
        setPdfPreview(null);
      });
      return () => {
        visible.current = false;
        epoch.current++;
        subscription.remove();
      };
    }, []),
  );
  async function document(record: WorkflowDocument, exporting: boolean) {
    const credential = state.pairing;
    if (
      !credential ||
      !page ||
      locked.current ||
      !visible.current ||
      AppState.currentState !== "active"
    )
      return;
    if (!exporting && record.bytes > 65_536) {
      setMessage({
        text: "This document exceeds the Reading preview limit. Export the verified original.",
        error: true,
      });
      return;
    }
    const ticket = epoch.current,
      started = scope;
    locked.current = true;
    setReading(true);
    setMessage(null);
    setPreview(null);
    setPdfPreview(null);
    const current = () =>
      mounted.current &&
      visible.current &&
      AppState.currentState === "active" &&
      epoch.current === ticket &&
      scopeRef.current === started;
    try {
      const bytes = await readGatewayWorkflowDocument(
        credential,
        state.ownerId,
        params.run,
        record,
        current,
        Crypto.randomUUID,
        async (bytes) => {
          const digest = new Uint8Array(
            await Crypto.digest(
              Crypto.CryptoDigestAlgorithm.SHA256,
              bytes.slice().buffer,
            ),
          );
          return [...digest]
            .map((byte) => byte.toString(16).padStart(2, "0"))
            .join("");
        },
      );
      if (!current()) return;
      if (exporting) {
        await exportWorkflowDocument(record, bytes, current);
        if (current())
          setMessage({ text: "Verified document exported.", error: false });
      } else if (record.format === "pdf") {
        setPdfPreview({
          document: record,
          revision: page.revision,
          scope: started,
          ticket,
        });
      } else {
        if (bytes.length > 65_536)
          throw new Error(
            "This document exceeds the Reading preview limit. Export the verified original.",
          );
        const text = workflowDocumentText(bytes);
        const sourceRows = documentPreviewRows(record.format, text, false);
        const readingPreview = await prepareDocumentReading(
          record.format,
          text,
          current,
        );
        if (!current()) return;
        setReadingView(true);
        setPreview({
          name: record.name,
          format: record.format,
          text,
          sourceRows,
          readingPreview,
          scope: started,
        });
      }
    } catch (error) {
      if (current())
        setMessage({
          text:
            error instanceof Error
              ? error.message
              : "Document unavailable. Refresh its evidence and try again.",
          error: true,
        });
    } finally {
      locked.current = false;
      if (mounted.current) setReading(false);
    }
  }
  // Cached rows belong only to the selected owner/run/credential generation.
  const activePreview =
    preview?.scope === scope &&
    visible.current &&
    AppState.currentState === "active"
      ? preview
      : null;
  const previewRows = activePreview
    ? readingView
      ? activePreview.readingPreview.rows
      : activePreview.sourceRows
    : [];
  const currentPdf = () =>
    !!pdfPreview &&
    mounted.current &&
    visible.current &&
    AppState.currentState === "active" &&
    epoch.current === pdfPreview.ticket &&
    scopeRef.current === pdfPreview.scope &&
    state.pairing !== null &&
    state.pairing.expiresAt > Date.now() &&
    state.pairing.grants.includes("evidenceRead") &&
    page?.revision === pdfPreview.revision &&
    !state.loading;
  if (pdfPreview && state.pairing && currentPdf())
    return (
      <ScrollView
        style={[styles.screen, { backgroundColor: theme.background }]}
        contentContainerStyle={{ paddingBottom: insets.bottom + 24 }}
      >
        <Stack.Screen options={{ title: pdfPreview.document.name }} />
        <View style={styles.header}>
          <Text style={[styles.title, { color: theme.text }]}>
            {pdfPreview.document.name}
          </Text>
          <View style={styles.actions}>
            <AutomationButton
              label="Back to documents"
              onPress={() => {
                epoch.current++;
                setPdfPreview(null);
              }}
            />
            <AutomationButton
              label="Export document"
              disabled={reading}
              onPress={() => void document(pdfPreview.document, true)}
            />
          </View>
          <PdfPageViewer
            source={state.pairing}
            identity={JSON.stringify([
              pdfPreview.document,
              pdfPreview.revision,
              pdfPreview.scope,
            ])}
            name={pdfPreview.document.name}
            current={currentPdf}
            read={(index, width, active) =>
              readGatewayPdfPage(
                state.pairing!,
                state.ownerId,
                params.run,
                pdfPreview.revision,
                pdfPreview.document,
                index,
                width,
                active,
                Crypto.randomUUID,
                pdfDigest,
              )
            }
          />
        </View>
      </ScrollView>
    );
  if (activePreview)
    return (
      <View style={[styles.screen, { backgroundColor: theme.background }]}>
        <Stack.Screen options={{ title: activePreview.name }} />
        <FlatList
          data={previewRows}
          keyExtractor={(row) => row.id}
          contentContainerStyle={{ paddingBottom: insets.bottom + 24 }}
          ListHeaderComponent={
            <View style={styles.header}>
              <Text style={[styles.title, { color: theme.text }]}>
                {activePreview.name}
              </Text>
              <Text style={[styles.body, { color: theme.textSecondary }]}>
                {readingView &&
                ["md", "json", "html", "htm"].includes(activePreview.format)
                  ? activePreview.readingPreview.sourceFallback
                    ? "Verified document · Source"
                    : "Verified document · Reading view"
                  : "Verified document · Source"}
              </Text>
              {["md", "json", "html", "htm"].includes(activePreview.format) && (
                <View style={styles.actions}>
                  <AutomationButton
                    label="Reading view"
                    disabled={readingView}
                    onPress={() => setReadingView(true)}
                  />
                  <AutomationButton
                    label="Source"
                    disabled={!readingView}
                    onPress={() => setReadingView(false)}
                  />
                </View>
              )}
              {["html", "htm"].includes(activePreview.format) && (
                <Text style={[styles.body, { color: theme.textSecondary }]}>
                  {readingView
                    ? activePreview.readingPreview.note
                    : "HTML Source · Scripts, links and media remain inactive."}
                </Text>
              )}
              <AutomationButton
                label="Back to documents"
                onPress={() => setPreview(null)}
              />
            </View>
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
  return (
    <View style={[styles.screen, { backgroundColor: theme.background }]}>
      <Stack.Screen options={{ title: "Workflow documents" }} />
      <FlatList
        data={page?.documents ?? []}
        keyExtractor={(doc) => doc.id}
        contentContainerStyle={{ paddingBottom: insets.bottom + 24 }}
        refreshControl={
          <RefreshControl
            refreshing={state.loading}
            onRefresh={() => {
              if (!reading) {
                epoch.current++;
                setPdfPreview(null);
                void state.refresh();
              }
            }}
            tintColor={theme.textSecondary}
          />
        }
        ListHeaderComponent={
          <View style={styles.header}>
            <AutomationRefresh
              refreshing={state.loading || reading}
              onRefresh={() => {
                epoch.current++;
                setPdfPreview(null);
                void state.refresh();
              }}
            />
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {page
                ? `${page.total} accepted generated documents`
                : "Accepted generated documents for this run"}
            </Text>
            {reading && (
              <Text
                accessibilityRole="alert"
                style={[styles.body, { color: theme.textSecondary }]}
              >
                Reading and verifying the complete document…
              </Text>
            )}
            {(message || state.message || !valid) && (
              <Text
                accessibilityRole="alert"
                style={[
                  styles.body,
                  {
                    color:
                      valid && !state.message && message?.error === false
                        ? theme.textSecondary
                        : theme.danger,
                  },
                ]}
              >
                {!valid
                  ? "Open documents from a registered workflow run."
                  : (message?.text ?? state.message)}
              </Text>
            )}
          </View>
        }
        ListEmptyComponent={
          <Text style={[styles.empty, { color: theme.textSecondary }]}>
            {state.loading
              ? "Loading documents…"
              : page
                ? "No accepted generated documents in this run."
                : "Refresh to read the document list."}
          </Text>
        }
        ListFooterComponent={
          <View style={[styles.actions, styles.footer]}>
            {previous.length > 0 && (
              <AutomationButton
                label="Previous"
                disabled={state.loading || reading || !page}
                onPress={() => {
                  setAfter(previous.at(-1) ?? null);
                  setPrevious(previous.slice(0, -1));
                }}
              />
            )}
            {page?.nextCursor && (
              <AutomationButton
                label="Next"
                disabled={state.loading || reading}
                onPress={() => {
                  setPrevious([...previous, after]);
                  setAfter(page.nextCursor);
                }}
              />
            )}
          </View>
        }
        renderItem={({ item: doc }) => (
          <View style={[styles.row, { borderBottomColor: theme.border }]}>
            <Text style={[styles.heading, { color: theme.text }]}>
              {doc.name}
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {doc.format.toUpperCase()} · {doc.bytes.toLocaleString()} bytes ·
              Stage {doc.stage + 1}
            </Text>
            <Text
              selectable
              style={[styles.body, { color: theme.textSecondary }]}
            >
              SHA-256 {doc.sha256}
            </Text>
            <View style={styles.actions}>
              {
                <AutomationButton
                  label="Preview"
                  disabled={state.loading || reading}
                  onPress={() => void document(doc, false)}
                />
              }
              <AutomationButton
                label="Export document"
                disabled={state.loading || reading}
                onPress={() => void document(doc, true)}
              />
            </View>
          </View>
        )}
      />
    </View>
  );
}
const styles = StyleSheet.create({
  screen: { flex: 1 },
  header: { padding: 20, gap: 12 },
  row: { padding: 20, gap: 10, borderBottomWidth: StyleSheet.hairlineWidth },
  title: { fontSize: 22, lineHeight: 30, fontWeight: "600" },
  heading: { fontSize: 17, lineHeight: 24, fontWeight: "600" },
  body: { fontSize: 14, lineHeight: 21 },
  empty: { padding: 24, fontSize: 15, lineHeight: 22 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  footer: { padding: 20 },
  source: {
    paddingHorizontal: 20,
    paddingBottom: 12,
    fontSize: 15,
    lineHeight: 24,
  },
});
