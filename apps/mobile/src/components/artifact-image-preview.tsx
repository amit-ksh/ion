import { useState } from "react";
import { Image, Text, useWindowDimensions, View } from "react-native";
import type { ArtifactImage } from "@/lib/artifact-image";
import { useTheme } from "@/hooks/use-theme";
/** Receives only verified prepared image bytes; owner scope is held by the parent. */
export function ArtifactImagePreview({
  image,
  name,
}: {
  image: ArtifactImage;
  name: string;
}) {
  const theme = useTheme(),
    screen = useWindowDimensions();
  const [failed, setFailed] = useState(false);
  return (
    <View style={{ backgroundColor: theme.background }}>
      {failed ? (
        <Text
          accessibilityRole="alert"
          style={{ color: theme.danger, padding: 16 }}
        >
          The image could not load. Export the verified original.
        </Text>
      ) : (
        <Image
          accessibilityLabel={name}
          accessible
          source={{ uri: image.uri }}
          resizeMode="contain"
          onError={() => setFailed(true)}
          style={{
            width: "100%",
            height: Math.max(
              160,
              Math.min(
                screen.height * 0.7,
                ((screen.width - 40) * image.height) / image.width,
              ),
            ),
          }}
        />
      )}
    </View>
  );
}
