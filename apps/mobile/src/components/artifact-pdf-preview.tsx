import type { ArtifactLibraryItem, WakuClient } from "@waku/client";
import * as Crypto from "expo-crypto";
import { useEffect, useRef, useState } from "react";
import {
  Image,
  ScrollView,
  StyleSheet,
  Text,
  useWindowDimensions,
  View,
} from "react-native";
import { AutomationButton } from "./automation-controls";
import { useTheme } from "@/hooks/use-theme";
import {
  PDF_ZOOMS,
  readArtifactPdfPage,
  type PdfPagePreview,
} from "@/lib/artifact-pdf";
/** Full original verification and owner scope are established by the parent. */
export function ArtifactPdfPreview({
  client,
  document,
  current,
}: {
  client: WakuClient;
  document: ArtifactLibraryItem;
  current: () => boolean;
}) {
  return (
    <PdfPageViewer
      source={client}
      identity={JSON.stringify(document)}
      name={document.name}
      current={current}
      read={(page, width, active) =>
        readArtifactPdfPage(client, document, page, width, active, pdfDigest)
      }
    />
  );
}
export async function pdfDigest(bytes: Uint8Array): Promise<string> {
  const hash = new Uint8Array(
    // Android's native digest takes a typed array, not a raw ArrayBuffer.
    await Crypto.digest(
      Crypto.CryptoDigestAlgorithm.SHA256,
      bytes as Uint8Array<ArrayBuffer>,
    ),
  );
  return [...hash].map((b) => b.toString(16).padStart(2, "0")).join("");
}
/** Mounted only after full original verification and while the parent owner scope is current. */
export function PdfPageViewer({
  source,
  identity,
  name,
  current,
  read,
}: {
  source: unknown;
  identity: string;
  name: string;
  current: () => boolean;
  read: (
    page: number,
    width: number,
    active: () => boolean,
  ) => Promise<PdfPagePreview>;
}) {
  const theme = useTheme(),
    screen = useWindowDimensions();
  const [selection, setSelection] = useState({
      page: 0,
      width: 800,
      refresh: 0,
    }),
    [loaded, setLoaded] = useState<{
      key: string;
      value: PdfPagePreview;
    } | null>(null),
    [message, setMessage] = useState<{ key: string; text: string } | null>(
      null,
    ),
    [busy, setBusy] = useState(false);
  const generation = useRef(0),
    mounted = useRef(true),
    currentRef = useRef(current),
    readRef = useRef(read);
  currentRef.current = current;
  readRef.current = read;
  const key = JSON.stringify([identity, selection]),
    keyRef = useRef(key);
  keyRef.current = key;
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      generation.current++;
    };
  }, []);
  useEffect(() => {
    const ticket = ++generation.current,
      started = key;
    const active = () =>
      mounted.current &&
      generation.current === ticket &&
      keyRef.current === started &&
      currentRef.current();
    setLoaded(null);
    setMessage(null);
    setBusy(true);
    void readRef
      .current(selection.page, selection.width, active)
      .then((value) => {
        if (active()) setLoaded({ key: started, value });
      })
      .catch((error) => {
        if (active())
          setMessage({
            key: started,
            text:
              error instanceof Error
                ? error.message
                : "PDF preview unavailable. Retry or export the original.",
          });
      })
      .finally(() => {
        if (active()) setBusy(false);
      });
    return () => {
      generation.current++;
    };
  }, [source, key, selection.page, selection.width]);
  const page = loaded?.key === key && current() ? loaded.value : null;
  const fit = Math.max(1, Math.min(1200, screen.width - 40)),
    displayWidth = page
      ? Math.min(
          (fit * selection.width) / 800,
          (4000 * page.width) / page.height,
        )
      : fit;
  function choose(index: number, width: number) {
    if (busy || !current()) return;
    generation.current++;
    setLoaded(null);
    setMessage(null);
    setSelection((s) => ({ page: index, width, refresh: s.refresh + 1 }));
  }
  return (
    <View style={styles.root}>
      <Text
        style={[styles.body, { color: theme.textSecondary }]}
        accessibilityLiveRegion="polite"
      >
        {busy
          ? "Rendering verified PDF page…"
          : page
            ? `Page ${page.page + 1} of ${page.pages}`
            : "PDF page preview"}
      </Text>
      <View style={styles.actions}>
        <AutomationButton
          label="Previous page"
          disabled={busy || !page || page.page === 0}
          onPress={() => choose(selection.page - 1, selection.width)}
        />
        <AutomationButton
          label="Next page"
          disabled={busy || !page || page.page + 1 >= page.pages}
          onPress={() => choose(selection.page + 1, selection.width)}
        />
      </View>
      <View style={styles.actions}>
        {PDF_ZOOMS.map((width) => (
          <AutomationButton
            key={width}
            label={width === 800 ? "Fit" : width === 1200 ? "150%" : "200%"}
            disabled={busy || selection.width === width}
            onPress={() => choose(selection.page, width)}
          />
        ))}
        {message?.key === key && (
          <AutomationButton
            label="Retry preview"
            disabled={busy}
            onPress={() => choose(selection.page, selection.width)}
          />
        )}
      </View>
      {message?.key === key && (
        <Text
          accessibilityRole="alert"
          style={[styles.body, { color: theme.danger }]}
        >
          {message.text}
        </Text>
      )}
      {page && (
        <ScrollView
          horizontal
          showsHorizontalScrollIndicator
          style={{ backgroundColor: theme.surfaceMuted }}
        >
          <Image
            key={key}
            source={{ uri: page.uri, cache: "reload" }}
            resizeMode="contain"
            accessibilityRole="image"
            accessibilityLabel={`${name}, page ${page.page + 1} of ${page.pages}`}
            style={{
              width: displayWidth,
              height: (displayWidth * page.height) / page.width,
            }}
            onError={() => {
              if (
                mounted.current &&
                keyRef.current === key &&
                currentRef.current()
              ) {
                generation.current++;
                setLoaded(null);
                setMessage({
                  key,
                  text: "The phone could not display this page. Retry or export the verified original.",
                });
              }
            }}
          />
        </ScrollView>
      )}
    </View>
  );
}
const styles = StyleSheet.create({
  root: { gap: 12 },
  actions: { flexDirection: "row", flexWrap: "wrap", gap: 8 },
  body: { fontSize: 14, lineHeight: 21 },
});
