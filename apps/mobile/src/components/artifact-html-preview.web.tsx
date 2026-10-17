import { useEffect, useRef, useState } from "react";
import { useWindowDimensions, Text, View } from "react-native";
import type { RenderedHtml } from "@/lib/html-rendering";
import { openHtmlResourceHost } from "@/lib/html-resource-host";
import { browserHtmlHost } from "@/lib/html-browser-host";
import { useTheme } from "@/hooks/use-theme";

export function ArtifactHtmlPreview({
  document,
  name,
  current,
}: {
  document: RenderedHtml;
  name: string;
  current?: () => boolean;
}) {
  const { height } = useWindowDimensions(),
    theme = useTheme();
  const scope = useRef(current);
  scope.current = current;
  const [host, setHost] = useState<{
    document: RenderedHtml;
    url: string;
  } | null>(null);
  const [error, setError] = useState(false);
  useEffect(() => {
    setHost(null);
    setError(false);
    if (!document.resources) return;
    let live = true,
      lease: Awaited<ReturnType<typeof openHtmlResourceHost>> | null = null,
      renewing = false;
    const active = () =>
      live &&
      globalThis.document.visibilityState === "visible" &&
      (scope.current?.() ?? true);
    const close = () => {
      live = false;
      setHost(null);
      void lease?.close().catch(() => {});
    };
    void browserHtmlHost()
      .then((adapter) =>
        openHtmlResourceHost(
          document.resources!,
          crypto.randomUUID(),
          active,
          adapter,
          location.origin,
        ),
      )
      .then((opened) => {
        lease = opened;
        if (!active()) {
          void opened.close().catch(() => {});
          return;
        }
        setHost({ document, url: opened.url });
      })
      .catch(() => {
        if (live) setError(true);
      });
    const timer = setInterval(() => {
      if (!active()) {
        close();
        return;
      }
      if (!lease || renewing) return;
      renewing = true;
      void lease
        .renew()
        .then((alive) => {
          if (!alive && live) {
            setError(true);
            close();
          }
        })
        .catch(() => {
          if (live) {
            setError(true);
            close();
          }
        })
        .finally(() => {
          renewing = false;
        });
    }, 5000);
    const visibility = () => {
      if (globalThis.document.visibilityState !== "visible") close();
    };
    globalThis.document.addEventListener("visibilitychange", visibility);
    window.addEventListener("pagehide", close);
    return () => {
      live = false;
      clearInterval(timer);
      globalThis.document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("pagehide", close);
      void lease?.close().catch(() => {});
    };
  }, [document]);
  const resourceUrl = host?.document === document ? host.url : null;
  return (
    <View style={{ height: Math.max(320, height * 0.65) }}>
      {error ? (
        <Text
          accessibilityRole="alert"
          style={{ color: theme.danger, padding: 16 }}
        >
          The rendered document could not load. Use Reading view or Source, or
          export the original.
        </Text>
      ) : document.resources && !resourceUrl ? null : (
        <iframe
          title={name}
          src={resourceUrl ?? undefined}
          srcDoc={document.resources ? undefined : document.html}
          sandbox={document.resources ? "allow-same-origin" : ""}
          referrerPolicy="no-referrer"
          style={{
            width: "100%",
            height: "100%",
            border: 0,
            backgroundColor: "white",
          }}
        />
      )}
    </View>
  );
}
