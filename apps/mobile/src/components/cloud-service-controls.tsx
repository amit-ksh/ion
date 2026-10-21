import { useEffect, useRef, useState } from "react";
import { Pressable, Switch, Text, View } from "react-native";
import { cloudAccount } from "@/lib/cloud-account";
import { useTheme } from "@/hooks/use-theme";
import { AppSymbol } from "./app-symbol";
type Controls = {
  cloud: boolean;
  tokenFactory: boolean;
  canManage: boolean;
  cloudAvailable?: boolean;
  tokenFactoryAvailable?: boolean;
};
function parse(value: unknown): Controls {
  const v = value as Partial<Controls> | null;
  if (
    !v ||
    typeof v.cloud !== "boolean" ||
    typeof v.tokenFactory !== "boolean" ||
    typeof v.canManage !== "boolean"
  )
    throw new Error("Invalid shared service status");
  return v as Controls;
}
export function CloudServiceControls() {
  const theme = useTheme(),
    mounted = useRef(false);
  const [state, setState] = useState<
      (Controls & { workspaceId: string }) | null
    >(null),
    [busy, setBusy] = useState(false),
    [feedback, setFeedback] = useState<string | null>(null);
  const [usage, setUsage] = useState<{
    tokenFactory?: { account?: { spent: number; held: number } };
    compute?: { state: string; note?: string | null };
  } | null>(null);
  const generation = useRef(0);
  const pending = useRef<number | null>(null);
  async function update(
    service?: "cloud" | "tokenFactory",
    enabled?: boolean,
    workspaceId?: string,
  ) {
    if (pending.current !== null) return;
    const operation = ++generation.current;
    const current = () => mounted.current && generation.current === operation;
    pending.current = operation;
    setBusy(true);
    setFeedback(null);
    try {
      // Switch callbacks carry the account from the rendered row. Initial and
      // explicit refreshes capture identity before starting either service read.
      const workspace = service
        ? workspaceId
        : (await cloudAccount.status()).workspaceId;
      if (!current()) return;
      if (!workspace)
        throw new Error("Sign in again to manage shared services.");
      const value = parse(
        await cloudAccount.serviceRequest(
          workspace,
          "/v1/services/controls",
          service ? "PUT" : "GET",
          service ? { [service]: enabled } : undefined,
        ),
      );
      if (current()) {
        setState({ ...value, workspaceId: workspace });
        const nextUsage = await cloudAccount
          .serviceRequest(workspace, "/v1/services/usage", "GET")
          .catch(() => null);
        if (!current()) return;
        setUsage(nextUsage as typeof usage);
        if (service === "cloud" && !enabled)
          setFeedback(
            "Cloud work is blocked. VM shutdown still needs confirmation from the controller.",
          );
      }
    } catch (error) {
      if (current()) {
        setState(null);
        setUsage(null);
        setFeedback(
          error instanceof Error
            ? error.message
            : "Could not update shared services. Retry.",
        );
      }
    } finally {
      if (pending.current === operation) pending.current = null;
      if (current()) setBusy(false);
    }
  }
  useEffect(() => {
    mounted.current = true;
    void update();
    const unsubscribeSignOut = cloudAccount.onSignOut(() => {
      ++generation.current;
      pending.current = null;
      setState(null);
      setUsage(null);
      setFeedback(null);
      setBusy(false);
    });
    const unsubscribe = cloudAccount.subscribe(() => {
      if (pending.current === null) void update();
    });
    return () => {
      mounted.current = false;
      ++generation.current;
      unsubscribeSignOut();
      unsubscribe();
    };
  }, []);
  return (
    <View
      style={{
        gap: 12,
        paddingVertical: 12,
        borderTopWidth: 1,
        borderColor: theme.borderStrong,
      }}
    >
      <View
        style={{
          flexDirection: "row",
          alignItems: "center",
          justifyContent: "space-between",
        }}
      >
        <Text style={{ color: theme.text, fontWeight: "600" }}>
          Shared services
        </Text>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel="Refresh shared services"
          disabled={busy}
          onPress={() => void update()}
          style={{
            minWidth: 44,
            minHeight: 44,
            alignItems: "center",
            justifyContent: "center",
            opacity: busy ? 0.5 : 1,
          }}
        >
          <AppSymbol
            name={{
              ios: "arrow.clockwise",
              android: "refresh",
              web: "refresh",
            }}
            size={20}
            tintColor={theme.text}
          />
        </Pressable>
      </View>
      {state ? (
        <>
          {(
            [
              ["cloud", "Nebius Cloud"],
              ["tokenFactory", "Token Factory"],
            ] as const
          ).map(([key, label]) => (
            <View
              key={key}
              style={{
                flexDirection: "row",
                alignItems: "center",
                justifyContent: "space-between",
                gap: 12,
              }}
            >
              <Text style={{ color: theme.text, flex: 1 }}>
                {label}: {state[key] ? "ON" : "OFF"}
              </Text>
              <Switch
                accessibilityLabel={"Shared " + label}
                value={state[key]}
                disabled={
                  busy ||
                  !state.canManage ||
                  (!state[key] &&
                    !(key === "cloud"
                      ? state.cloudAvailable
                      : state.tokenFactoryAvailable))
                }
                onValueChange={(enabled) =>
                  void update(key, enabled, state.workspaceId)
                }
              />
            </View>
          ))}
          <Text style={{ color: theme.textSecondary, lineHeight: 20 }}>
            Shared services use owner-configured spending limits. Cloud ON
            permits on-demand work while the operator controller is online. A
            configured personal key is used first, with its own billing.
          </Text>
          {!state.canManage && (
            <Text style={{ color: theme.textSecondary }}>
              Managed by the owner.
            </Text>
          )}
        </>
      ) : (
        <Text style={{ color: theme.textSecondary }}>
          {busy
            ? "Checking shared services…"
            : "Shared service status is unavailable."}
        </Text>
      )}
      {usage?.tokenFactory?.account && (
        <Text style={{ color: theme.textSecondary }}>
          Shared inference: $
          {(usage.tokenFactory.account.spent / 1_000_000).toFixed(4)} used · $
          {(usage.tokenFactory.account.held / 1_000_000).toFixed(4)} reserved ·
          Cloud: {usage.compute?.state ?? "UNKNOWN"}
        </Text>
      )}
      {usage?.compute?.note && (
        <Text style={{ color: theme.textSecondary }}>{usage.compute.note}</Text>
      )}
      {feedback && (
        <Text
          accessibilityLiveRegion="polite"
          style={{ color: theme.textSecondary, lineHeight: 20 }}
        >
          {feedback}
        </Text>
      )}
    </View>
  );
}
