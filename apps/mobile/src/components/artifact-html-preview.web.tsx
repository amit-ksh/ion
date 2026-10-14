import { useWindowDimensions, View } from "react-native";
import type { RenderedHtml } from "@/lib/html-rendering";
export function ArtifactHtmlPreview({
  document,
  name,
}: {
  document: RenderedHtml;
  name: string;
}) {
  const { height } = useWindowDimensions();
  return (
    <View style={{ height: Math.max(320, height * 0.65) }}>
      <iframe
        title={name}
        srcDoc={document.html}
        sandbox=""
        referrerPolicy="no-referrer"
        style={{
          width: "100%",
          height: "100%",
          border: 0,
          backgroundColor: "white",
        }}
      />
    </View>
  );
}
