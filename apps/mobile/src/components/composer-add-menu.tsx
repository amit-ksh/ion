import { MenuView, type MenuAction } from "@expo/ui/community/menu";
import { StyleSheet, View } from "react-native";

import { AppSymbol } from "./app-symbol";
import { Radius } from "@/constants/theme";
import { useTheme } from "@/hooks/use-theme";

export type ComposerAttachmentSource = "files" | "camera" | "photo";

const AttachmentActions: MenuAction[] = [
  // Phone attachments, named apart from the desktop files mentioned above.
  { id: "files", title: "Upload from phone", image: "folder" },
  { id: "camera", title: "Take photo", image: "camera" },
  { id: "photo", title: "Choose photo", image: "photo" },
];

export function ComposerAddMenu({
  disabled = false,
  onChoose,
  onChooseContext,
}: {
  disabled?: boolean;
  onChoose?: (source: ComposerAttachmentSource) => void;
  onChooseContext: (kind: "command" | "file" | "team" | "app") => void;
}) {
  const theme = useTheme();
  const trigger = (
    <View
      accessible
      accessibilityLabel="Add to message"
      accessibilityRole="button"
      accessibilityState={{ disabled }}
      style={[styles.trigger, { opacity: disabled ? 0.35 : 1 }]}
    >
      <View style={[styles.disc, { backgroundColor: theme.overlay, borderColor: theme.border }]}>
        <AppSymbol
          name={{ ios: "plus", android: "add", web: "add" }}
          size={20}
          tintColor={theme.textSecondary}
        />
      </View>
    </View>
  );
  if (disabled) return trigger;

  return (
    <MenuView
      actions={[
        { id: "command", title: "Commands", image: "command" },
        { id: "file", title: "Add files, teams or apps", image: "at" },
        { id: "team", title: "Add an AI team", image: "person.3" },
        { id: "app", title: "Add an app", image: "square.grid.2x2" },
        ...(onChoose ? AttachmentActions : []),
      ]}
      onPressAction={({ nativeEvent }) => {
        const source = nativeEvent.event;
        // Let the native menu finish dismissing before presenting a picker.
        setTimeout(() => {
          if (
            source === "command" ||
            source === "file" ||
            source === "team" ||
            source === "app"
          )
            onChooseContext(source);
          else if (
            source === "files" ||
            source === "camera" ||
            source === "photo"
          )
            onChoose?.(source);
        }, 160);
      }}
    >
      {trigger}
    </MenuView>
  );
}

const styles = StyleSheet.create({
  trigger: {
    alignItems: "center",
    height: 48,
    justifyContent: "center",
    width: 48,
  },
  disc: {
    alignItems: "center",
    borderRadius: Radius.pill,
    borderWidth: StyleSheet.hairlineWidth,
    height: 36,
    justifyContent: "center",
    width: 36,
  },
});
