import { useEffect, useRef, useState } from "react";
import { requireOptionalNativeModule } from "expo";
import * as Crypto from "expo-crypto";
import {
  allowResourceNavigation,
  openHtmlResourceHost,
  type HtmlHostAdapter,
} from "@/lib/html-resource-host";
import {
  AppState,
  Platform,
  Text,
  View,
  useWindowDimensions,
} from "react-native";
import { WebView } from "react-native-webview";
import {
  allowDocumentNavigation,
  type RenderedHtml,
} from "@/lib/html-rendering";
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
  const theme = useTheme(),
    { height } = useWindowDimensions();
  const [error, setError] = useState(false);
  const [host, setHost] = useState<{
    document: RenderedHtml;
    url: string;
  } | null>(null);
  const scope = useRef(current);
  scope.current = current;
  useEffect(() => {
    setError(false);
    setHost(null);
    if (!document.resources) return;
    let live = true,
      lease: Awaited<ReturnType<typeof openHtmlResourceHost>> | null = null,
      renewing = false;
    const active = () =>
      live && AppState.currentState === "active" && (scope.current?.() ?? true);
    const close = () => {
      live = false;
      setHost(null);
      void lease?.close().catch(() => {});
    };
    const native =
      requireOptionalNativeModule<HtmlHostAdapter>("IonDocumentHost");
    if (!native) {
      setError(true);
      return;
    }
    void openHtmlResourceHost(
      document.resources,
      Crypto.randomUUID(),
      active,
      native,
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
    const subscription = AppState.addEventListener("change", (state) => {
      if (state !== "active") close();
    });
    return () => {
      live = false;
      clearInterval(timer);
      subscription.remove();
      void lease?.close().catch(() => {});
    };
  }, [document]);
  const resourceUrl = host?.document === document ? host.url : null;
  return (
    <View
      style={{
        height: Math.max(320, height * 0.65),
        backgroundColor: theme.background,
      }}
    >
      {error ? (
        <Text
          accessibilityRole="alert"
          style={{ color: theme.danger, padding: 16 }}
        >
          The rendered document could not load. Use Reading view or Source, or
          export the original.
        </Text>
      ) : document.resources && !resourceUrl ? null : (
        <WebView
          accessibilityLabel={name}
          source={
            resourceUrl
              ? { uri: resourceUrl }
              : { html: document.html, baseUrl: "about:blank" }
          }
          originWhitelist={["*"]}
          onShouldStartLoadWithRequest={(request) =>
            resourceUrl
              ? allowResourceNavigation(request.url, resourceUrl)
              : allowDocumentNavigation(request.url)
          }
          javaScriptEnabled={false}
          domStorageEnabled={false}
          allowFileAccess={false}
          allowFileAccessFromFileURLs={false}
          allowUniversalAccessFromFileURLs={false}
          mixedContentMode="never"
          incognito
          cacheEnabled={false}
          sharedCookiesEnabled={false}
          thirdPartyCookiesEnabled={false}
          javaScriptCanOpenWindowsAutomatically={false}
          setSupportMultipleWindows
          onOpenWindow={() => {}}
          allowsInlineMediaPlayback={false}
          mediaPlaybackRequiresUserAction
          allowsLinkPreview={false}
          dataDetectorTypes="none"
          contentMode="mobile"
          webviewDebuggingEnabled={false}
          onError={() => setError(true)}
          onContentProcessDidTerminate={() => setError(true)}
          onRenderProcessGone={() => setError(true)}
          nestedScrollEnabled={Platform.OS === "android"}
          style={{ flex: 1, backgroundColor: "white" }}
        />
      )}
    </View>
  );
}
