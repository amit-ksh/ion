import {
  Image,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from "react-native";

import { CloudAccountSection } from "./cloud-account-section";
import { AppSymbol } from "@/components/app-symbol";
import { Radius } from "@/constants/theme";
import { useTheme } from "@/hooks/use-theme";

export function DaemonOnboarding({
  onAddDaemon,
  onPairGateway,
}: {
  onAddDaemon: () => void;
  onPairGateway: () => void;
}) {
  const theme = useTheme();
  return (
    <ScrollView
      style={{ backgroundColor: theme.background }}
      contentContainerStyle={styles.screen}
      contentInsetAdjustmentBehavior="automatic"
    >
      <Image
        accessibilityLabel="ion"
        source={require("@/assets/images/icon.png")}
        style={styles.appIcon}
      />
      <Text style={[styles.title, { color: theme.text }]}>
        Your agents, everywhere.
      </Text>
      <Text style={[styles.body, { color: theme.textSecondary }]}>
        Connect to ion running on your Mac, workstation, or private server. Add
        more than one and switch whenever you need.
      </Text>
      <View style={styles.account}>
        <CloudAccountSection />
      </View>
      <Pressable
        accessibilityLabel="Add a daemon"
        accessibilityRole="button"
        onPress={onAddDaemon}
        style={({ pressed }) => [
          styles.primaryButton,
          { backgroundColor: theme.primary, opacity: pressed ? 0.78 : 1 },
        ]}
      >
        <AppSymbol
          name={{ ios: "plus", android: "add", web: "add" }}
          size={17}
          tintColor={theme.onPrimary}
        />
        <Text style={[styles.primaryButtonText, { color: theme.onPrimary }]}>
          Add a daemon
        </Text>
      </Pressable>
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Pair personal gateway"
        onPress={onPairGateway}
        style={styles.gatewayButton}
      >
        <Text style={[styles.gatewayButtonText, { color: theme.text }]}>
          Pair personal gateway
        </Text>
      </Pressable>
      <View style={styles.securityNote}>
        <AppSymbol
          name={{ ios: "lock.shield", android: "shield_lock", web: "lock" }}
          size={15}
          tintColor={theme.textTertiary}
        />
        <Text style={[styles.securityText, { color: theme.textTertiary }]}>
          Tokens stay on this device and go directly to the host you choose.
        </Text>
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  screen: {
    alignItems: "center",
    flexGrow: 1,
    justifyContent: "center",
    paddingTop: 40,
    paddingBottom: 48,
    paddingHorizontal: 34,
  },
  account: { width: "100%", maxWidth: 440, marginTop: 24 },
  appIcon: { borderRadius: 18, height: 72, marginBottom: 24, width: 72 },
  title: {
    fontSize: 28,
    fontWeight: "700",
    letterSpacing: -0.7,
    textAlign: "center",
  },
  body: {
    fontSize: 16,
    lineHeight: 23,
    marginTop: 10,
    maxWidth: 440,
    textAlign: "center",
  },
  primaryButton: {
    alignItems: "center",
    borderRadius: Radius.pill,
    flexDirection: "row",
    gap: 8,
    marginTop: 28,
    minHeight: 50,
    paddingHorizontal: 22,
  },
  primaryButtonText: { fontSize: 16, fontWeight: "700" },
  gatewayButton: {
    justifyContent: "center",
    minHeight: 48,
    marginTop: 10,
    paddingHorizontal: 18,
  },
  gatewayButtonText: { fontSize: 15, fontWeight: "600" },
  securityNote: {
    alignItems: "flex-start",
    flexDirection: "row",
    gap: 8,
    marginTop: 24,
    maxWidth: 330,
  },
  securityText: { flex: 1, fontSize: 12, lineHeight: 17 },
});
