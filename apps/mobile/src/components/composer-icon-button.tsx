import { Pressable, StyleSheet, View } from "react-native";

import { AppSymbol } from "./app-symbol";
import { Radius } from "@/constants/theme";
import { useTheme } from "@/hooks/use-theme";

/** Round icon control in the composer toolbar, with a 48dp touch target. */
export function ComposerIconButton({
  icon,
  label,
  onPress,
  active = false,
  disabled = false,
}: {
  icon: Parameters<typeof AppSymbol>[0]["name"];
  label: string;
  onPress: () => void;
  active?: boolean;
  disabled?: boolean;
}) {
  const theme = useTheme();
  return (
    <Pressable
      accessibilityLabel={label}
      accessibilityRole="button"
      accessibilityState={{ disabled, selected: active }}
      disabled={disabled}
      hitSlop={4}
      onPress={onPress}
      style={[styles.iconButton, { opacity: disabled ? 0.35 : 1 }]}>
      {({ pressed }) => (
        <View collapsable={false} style={[styles.iconDisc, { backgroundColor: active ? theme.accentSoft : pressed ? theme.overlayStrong : "transparent" }]}>
          <AppSymbol name={icon} size={20} tintColor={active ? theme.accent : theme.textSecondary} />
        </View>
      )}
    </Pressable>
  );
}

const styles = StyleSheet.create({
  iconButton: {
    alignItems: "center",
    height: 48,
    justifyContent: "center",
    width: 48,
  },
  iconDisc: {
    alignItems: "center",
    borderRadius: Radius.pill,
    height: 40,
    justifyContent: "center",
    width: 40,
  },
});
