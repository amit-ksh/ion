import { useState } from "react";
import { Platform, Text, View, useWindowDimensions } from "react-native";
import { WebView } from "react-native-webview";
import {
  allowDocumentNavigation,
  type RenderedHtml,
} from "@/lib/html-rendering";
import { useTheme } from "@/hooks/use-theme";

export function ArtifactHtmlPreview({
  document,
  name,
}: {
  document: RenderedHtml;
  name: string;
}) {
  const theme = useTheme(),
    { height } = useWindowDimensions();
  const [error, setError] = useState(false);
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
      ) : (
        <WebView
          accessibilityLabel={name}
          source={{ html: document.html, baseUrl: "about:blank" }}
          originWhitelist={["*"]}
          onShouldStartLoadWithRequest={(request) =>
            allowDocumentNavigation(request.url)
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
