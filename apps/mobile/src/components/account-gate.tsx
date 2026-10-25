import { useQueryClient } from "@tanstack/react-query";
import * as SplashScreen from "expo-splash-screen";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  ActivityIndicator,
  AppState,
  StyleSheet,
  Text,
  View,
} from "react-native";
import { cloudAccount } from "@/lib/cloud-account";
import type { CloudAccountStatus } from "@/lib/cloud-account.types";
import { useTheme } from "@/hooks/use-theme";
import { GlowField } from "./glow-field";
import { IonMarkTile } from "./ion-mark";
import { WorkbenchButton } from "./workbench";
import { MonoFont } from "@/constants/theme";

/** No route or daemon/runtime provider mounts before a verified account is ready. */
export function AccountGate({ children }: { children: ReactNode }) {
  const theme = useTheme();
  const queryClient = useQueryClient();
  const [status, setStatus] = useState<CloudAccountStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [covered, setCovered] = useState(true);
  const [problem, setProblem] = useState<string | null>(null);
  const alive = useRef(true);
  const generation = useRef(0);
  const checking = useRef(false);
  const workspace = useRef<string | undefined>(undefined);
  const reload = useCallback(
    async (cover = false) => {
      if (checking.current) return;
      checking.current = true;
      const run = ++generation.current;
      if (cover) setCovered(true);
      try {
        const next = await cloudAccount.verifySession();
        if (!alive.current || run !== generation.current) return;
        if (workspace.current !== next.workspaceId || !next.signedIn) {
          void queryClient.cancelQueries();
          queryClient.clear();
        }
        workspace.current = next.workspaceId;
        setStatus(next);
        setProblem(next.problem ?? null);
      } catch {
        if (!alive.current || run !== generation.current) return;
        setStatus({ available: true, signedIn: false });
        setProblem(
          "Could not verify your account. Check your connection and retry, or sign in again.",
        );
        void queryClient.cancelQueries();
        queryClient.clear();
      } finally {
        if (run === generation.current) {
          checking.current = false;
          if (alive.current) setCovered(false);
        }
        if (alive.current) void SplashScreen.hideAsync();
      }
    },
    [queryClient],
  );
  useEffect(() => {
    alive.current = true;
    void reload(true);
    const unsubscribe = cloudAccount.subscribe(() => {
      void reload(true);
    });
    const signOut = cloudAccount.onSignOut(() => {
      generation.current++;
      checking.current = false;
      workspace.current = undefined;
      setStatus({ available: true, signedIn: false });
      setCovered(false);
      void queryClient.cancelQueries();
      queryClient.clear();
    });
    const active = AppState.addEventListener("change", (value) => {
      if (value === "active") void reload(true);
    });
    const timer = setInterval(() => {
      if (AppState.currentState === "active") void reload();
    }, 60_000);
    return () => {
      alive.current = false;
      generation.current++;
      unsubscribe();
      signOut();
      active.remove();
      clearInterval(timer);
    };
  }, [queryClient, reload]);
  async function signIn() {
    if (busy) return;
    setBusy(true);
    setProblem(null);
    try {
      if (await cloudAccount.signIn()) await reload(true);
    } catch (error) {
      if (alive.current)
        setProblem(
          error instanceof Error
            ? error.message
            : "Could not sign in. Try again.",
        );
    } finally {
      if (alive.current) setBusy(false);
    }
  }
  const ready = Boolean(status?.signedIn && status.workspaceId);
  return (
    <View style={[styles.root, { backgroundColor: theme.background }]}>
      {ready && (
        <View
          key={status!.workspaceId}
          style={[styles.root, covered && { opacity: 0 }]}
          pointerEvents={covered ? "none" : "auto"}
          accessibilityElementsHidden={covered}
          importantForAccessibility={covered ? "no-hide-descendants" : "auto"}
        >
          {children}
        </View>
      )}
      {(!ready || covered) && (
        <View
          style={[
            styles.screen,
            StyleSheet.absoluteFill,
            { backgroundColor: theme.background },
          ]}
        >
          <GlowField />
          <View style={styles.content}>
            <IonMarkTile size={64} />
            <Text accessibilityRole="header" style={[styles.heading, { color: theme.text }]}>
              Sign in to Ion
            </Text>
            <Text style={[styles.body, { color: theme.textSecondary }]}>
              {status === null || covered
                ? "Checking your account…"
                : status.available
                  ? "Sign in securely in your browser to open your workspace."
                  : "Sign-in is not configured in this build. Install an updated Ion build connected to Ion Cloud."}
            </Text>
            {status === null || covered ? (
              <ActivityIndicator
                accessibilityLabel="Checking account"
                color={theme.textSecondary}
              />
            ) : (
              <View style={styles.actions}>
                {status.available && (
                  <WorkbenchButton
                    disabled={busy}
                    label={busy ? "Opening sign-in…" : "Sign in"}
                    onPress={() => void signIn()}
                    primary
                  />
                )}
                <WorkbenchButton
                  disabled={busy}
                  label="Retry"
                  onPress={() => void reload(true)}
                />
              </View>
            )}
            {problem && (
              <Text
                accessibilityLiveRegion="polite"
                style={[styles.body, { color: theme.textSecondary }]}
              >
                {problem}
              </Text>
            )}
          </View>
        </View>
      )}
    </View>
  );
}
const styles = StyleSheet.create({
  root: { flex: 1 },
  screen: {
    flex: 1,
    justifyContent: "center",
    alignItems: "center",
    padding: 28,
  },
  content: { width: "100%", maxWidth: 420, gap: 16, alignItems: "center" },
  heading: { fontFamily: MonoFont, fontSize: 28, lineHeight: 36, fontWeight: "500", textAlign: "center", marginTop: 8 },
  body: { fontSize: 15.5, lineHeight: 23, textAlign: "center" },
  actions: { alignSelf: "stretch", gap: 10, marginTop: 8 },
});
