import { useCallback, useEffect, useRef, useState } from "react";
import {
  ActivityIndicator,
  Alert,
  Pressable,
  StyleSheet,
  Switch,
  Text,
  View,
} from "react-native";
import { router } from "expo-router";
import { AppSymbol } from "./app-symbol";
import { CloudServiceControls } from "./cloud-service-controls";
import { cloudAccount } from "@/lib/cloud-account";
import type { CloudAccountStatus } from "@/lib/cloud-account.types";
import { useTheme } from "@/hooks/use-theme";

export function CloudAccountSection() {
  const theme = useTheme();
  const mounted = useRef(true);
  const [status, setStatus] = useState<CloudAccountStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [controls, setControls] = useState(false);
  const [feedback, setFeedback] = useState<string | null>(null);
  const reload = useCallback(() => {
    void cloudAccount
      .status()
      .then((next) => {
        if (mounted.current) {
          setStatus(next);
          setControls(next.controlsEnabled ?? false);
        }
      })
      .catch(() => {
        if (mounted.current)
          setFeedback("Could not read your account. Try again.");
      });
  }, []);
  useEffect(() => {
    mounted.current = true;
    reload();
    const unsubscribe = cloudAccount.subscribe(reload);
    return () => {
      mounted.current = false;
      unsubscribe();
    };
  }, [reload]);

  async function run(action: "signIn" | "connect" | "signOut") {
    if (busy) return;
    setBusy(true);
    setFeedback(null);
    try {
      if (action === "signIn") {
        if (!(await cloudAccount.signIn())) return;
      } else if (action === "connect") {
        await cloudAccount.connectPhone(controls);
        router.push("/gateway");
      } else {
        const result = await cloudAccount.signOut();
        if (!result.revoked)
          Alert.alert(
            "Signed out on this phone",
            "Remote revocation could not be confirmed. Review this device when you reconnect.",
          );
        if (mounted.current)
          setFeedback(
            result.revoked
              ? "Signed out."
              : "Signed out on this phone. Remote revocation could not be confirmed; review this device when you reconnect.",
          );
      }
      reload();
    } catch (error) {
      if (action === "signIn")
        Alert.alert(
          "Could not sign in",
          "Check your connection and try again. If this continues, review the Ion Cloud setup.",
        );
      if (mounted.current)
        setFeedback(
          error instanceof Error
            ? error.message
            : "Could not complete this action. Try again.",
        );
    } finally {
      if (mounted.current) setBusy(false);
    }
  }
  function button(
    label: string,
    action: "signIn" | "connect" | "signOut",
    primary = false,
  ) {
    return (
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={label}
        accessibilityState={{ disabled: busy }}
        disabled={busy}
        onPress={() => void run(action)}
        style={({ pressed }) => [
          styles.button,
          {
            backgroundColor: primary ? theme.inverse : theme.background,
            borderColor: theme.borderStrong,
            opacity: busy || pressed ? 0.65 : 1,
          },
        ]}
      >
        <AppSymbol
          name={{
            ios:
              action === "signOut"
                ? "rectangle.portrait.and.arrow.right"
                : "person.crop.circle",
            android: action === "signOut" ? "logout" : "account_circle",
            web: action === "signOut" ? "logout" : "account_circle",
          }}
          size={19}
          tintColor={primary ? theme.onInverse : theme.text}
        />
        <Text
          style={[
            styles.buttonText,
            { color: primary ? theme.onInverse : theme.text },
          ]}
        >
          {label}
        </Text>
      </Pressable>
    );
  }
  return (
    <View style={[styles.section, { borderColor: theme.borderStrong }]}>
      <View style={styles.heading}>
        <AppSymbol
          name={{ ios: "cloud", android: "cloud", web: "cloud" }}
          size={20}
          tintColor={theme.text}
        />
        <Text style={[styles.title, { color: theme.text }]}>Ion Cloud</Text>
        {busy && (
          <ActivityIndicator
            size="small"
            color={theme.textSecondary}
            accessibilityLabel="Updating cloud account"
          />
        )}
      </View>
      {status === null ? (
        <Text style={[styles.body, { color: theme.textSecondary }]}>
          Checking account…
        </Text>
      ) : !status.available ? (
        <Text style={[styles.body, { color: theme.textSecondary }]}>
          Cloud sign-in is not available in this build yet. You can connect
          directly to your computer below.
        </Text>
      ) : status.signedIn ? (
        <>
          <Text style={[styles.body, { color: theme.textSecondary }]}>
            {status.phoneConnected
              ? "This phone is connected."
              : "Signed in. Connect this phone to view work from your linked computers."}
          </Text>
          <View style={styles.controlRow}>
            <View style={styles.controlCopy}>
              <Text style={[styles.label, { color: theme.text }]}>
                Allow task controls
              </Text>
              <Text style={[styles.help, { color: theme.textSecondary }]}>
                Stop and steer tasks, review approvals, and manage workflows and
                routines.
              </Text>
            </View>
            <Switch
              accessibilityLabel="Allow task controls on this phone"
              value={controls}
              disabled={busy}
              onValueChange={setControls}
            />
          </View>
          {button(
            status.phoneConnected
              ? "Update phone access"
              : "Connect this phone",
            "connect",
            true,
          )}
          {status.phoneConnected && (
            <Pressable
              accessibilityRole="button"
              onPress={() => router.push("/gateway")}
              style={styles.link}
            >
              <Text style={[styles.label, { color: theme.text }]}>
                Open tasks and approvals
              </Text>
            </Pressable>
          )}
          <CloudServiceControls key={status.workspaceId} />
          {button("Sign out", "signOut")}
        </>
      ) : (
        <>
          <Text style={[styles.body, { color: theme.textSecondary }]}>
            Sign in to access work from computers linked to your account.
          </Text>
          {button("Sign in to Ion Cloud", "signIn", true)}
        </>
      )}
      {status?.problem && (
        <Text
          accessibilityRole="alert"
          style={[styles.body, { color: theme.text }]}
        >
          {status.problem}
        </Text>
      )}
      {status?.signedIn && (
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Open deployments"
          style={{ minHeight: 44, justifyContent: "center" }}
          onPress={() => router.push("/cloud-deployments")}
        >
          <Text style={{ color: theme.text, fontWeight: "600" }}>
            Deployments
          </Text>
        </Pressable>
      )}
      {feedback && (
        <Text
          accessibilityRole="alert"
          accessibilityLiveRegion="polite"
          style={[styles.body, { color: theme.text }]}
        >
          {feedback}
        </Text>
      )}
    </View>
  );
}
const styles = StyleSheet.create({
  section: {
    alignSelf: "stretch",
    borderWidth: StyleSheet.hairlineWidth,
    borderRadius: 12,
    padding: 16,
    gap: 12,
    marginBottom: 18,
    width: "100%",
    maxWidth: 560,
  },
  heading: { flexDirection: "row", alignItems: "center", gap: 9 },
  title: { fontSize: 18, fontWeight: "600", flex: 1 },
  body: { fontSize: 14, lineHeight: 21 },
  button: {
    minHeight: 48,
    flexDirection: "row",
    alignItems: "center",
    justifyContent: "center",
    gap: 9,
    paddingHorizontal: 16,
    paddingVertical: 10,
    borderRadius: 9,
    borderWidth: StyleSheet.hairlineWidth,
  },
  buttonText: { fontSize: 15, fontWeight: "600", flexShrink: 1 },
  controlRow: { flexDirection: "row", alignItems: "center", gap: 14 },
  controlCopy: { flex: 1, gap: 4 },
  label: { fontSize: 14, fontWeight: "500" },
  help: { fontSize: 13, lineHeight: 19 },
  link: { minHeight: 44, justifyContent: "center" },
});
