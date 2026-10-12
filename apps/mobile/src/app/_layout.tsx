import {
  DarkTheme,
  DefaultTheme,
  Stack,
  router,
  ThemeProvider,
  type NativeStackNavigationOptions,
} from "expo-router";
import * as SplashScreen from "expo-splash-screen";
import { StatusBar } from "expo-status-bar";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect, useMemo } from "react";
import { StyleSheet, useColorScheme } from "react-native";
import { GestureHandlerRootView } from "react-native-gesture-handler";

import { Colors } from "@/constants/theme";
import {
  HeaderAction,
  HeaderActionGroup,
  nativeHeaderButtons,
  type HeaderActionSpec,
} from "@/components/screen-header";
import { TaskDrawerHost, useTaskDrawer } from "@/components/task-drawer";
import { DaemonProvider, useDaemon } from "@/lib/daemon-context";
import { RuntimeProvider } from "@/lib/runtime-context";
import { listenForGatewayNotifications } from "@/lib/gateway-notifications";

/** Deep links and state restores keep the new-task home as the stack anchor. */
export const unstable_settings = { anchor: "index" };

void SplashScreen.preventAutoHideAsync();
SplashScreen.setOptions({ duration: 250, fade: true });

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnReconnect: true,
    },
  },
});

/**
 * Native navigation bar with no background of its own: content scrolls
 * beneath it and its Liquid Glass items float. Every screen on the main path
 * shows the bar, the task list included, because that is what lets UIKit hold
 * the bar's buttons in place and crossfade them during an interactive
 * swipe-back; popping to a screen without a bar makes UIKit slide the whole
 * bar out with the page instead.
 */
const floatingHeader = {
  headerShown: true,
  headerStyle: { backgroundColor: "transparent" },
  headerTransparent: true,
} satisfies NativeStackNavigationOptions;

export default function RootLayout() {
  useEffect(
    () =>
      listenForGatewayNotifications((destination) => {
        if (destination.scope === "inbox") router.push("/gateway-portfolio");
        else
          router.push({
            pathname: "/gateway-task/[id]",
            params: { id: destination.taskId },
          });
      }),
    [],
  );
  const colorScheme = useColorScheme();
  const colors = Colors[colorScheme === "dark" ? "dark" : "light"];
  const navigationTheme =
    colorScheme === "dark"
      ? {
          ...DarkTheme,
          colors: {
            ...DarkTheme.colors,
            background: colors.background,
            card: colors.background,
          },
        }
      : {
          ...DefaultTheme,
          colors: {
            ...DefaultTheme.colors,
            background: colors.background,
            card: colors.background,
          },
        };
  return (
    <GestureHandlerRootView style={styles.root}>
      <QueryClientProvider client={queryClient}>
        <DaemonProvider>
          <RuntimeProvider>
            <ThemeProvider value={navigationTheme}>
              <TaskDrawerHost>
                <AppNavigator />
              </TaskDrawerHost>
              <StatusBar style="auto" />
            </ThemeProvider>
          </RuntimeProvider>
        </DaemonProvider>
      </QueryClientProvider>
    </GestureHandlerRootView>
  );
}

function AppNavigator() {
  const { phase, profiles } = useDaemon();
  const { openTaskDrawer } = useTaskDrawer();
  const colorScheme = useColorScheme();
  const theme = Colors[colorScheme === "dark" ? "dark" : "light"];
  const daemonRoutesAvailable = phase === "booting" || profiles.length > 0;
  const drawerAction = useMemo<HeaderActionSpec>(
    () => ({
      icon: { ios: "sidebar.left", android: "menu", web: "menu" },
      label: "Task history",
      onPress: openTaskDrawer,
    }),
    [openTaskDrawer],
  );
  const drawerHeader = useMemo<NativeStackNavigationOptions>(
    () => ({
      ...floatingHeader,
      gestureEnabled: false,
      headerLeft: () => (
        <HeaderActionGroup>
          <HeaderAction {...drawerAction} />
        </HeaderActionGroup>
      ),
      unstable_headerLeftItems: () => nativeHeaderButtons([drawerAction]),
    }),
    [drawerAction],
  );

  useEffect(() => {
    if (phase !== "booting") void SplashScreen.hideAsync();
  }, [phase]);

  return (
    <Stack
      screenOptions={{
        contentStyle: { backgroundColor: theme.background },
        headerBackButtonDisplayMode: "minimal",
        headerShadowVisible: false,
        headerStyle: { backgroundColor: theme.background },
        headerTintColor: theme.text,
      }}
    >
      <Stack.Screen
        name="index"
        options={
          daemonRoutesAvailable
            ? { ...drawerHeader, title: "New Task" }
            : { headerShown: false, title: "ion" }
        }
      />
      {/* Removing the final saved daemon also removes every daemon-backed
       * route from navigation, returning restored and open tasks to home. */}
      <Stack.Screen name="speech" options={{ title: "Cloud speech" }} />
      <Stack.Protected guard={daemonRoutesAvailable}>
        <Stack.Screen
          name="artifacts"
          options={{ ...drawerHeader, title: "Artifacts" }}
        />
        <Stack.Screen
          name="artifact-document"
          options={{ ...floatingHeader, title: "Document" }}
        />
        <Stack.Screen
          name="usage"
          options={{ ...drawerHeader, title: "Usage" }}
        />
        <Stack.Screen
          name="inbox-source"
          options={{ ...floatingHeader, title: "Review" }}
        />
        <Stack.Screen
          name="cloud-budgets"
          options={{ ...floatingHeader, title: "GPU budgets" }}
        />
        <Stack.Screen
          name="inbox"
          options={{ ...drawerHeader, title: "Inbox" }}
        />
        <Stack.Screen
          name="automations"
          options={{ ...drawerHeader, title: "Workflows & routines" }}
        />
        <Stack.Screen
          name="automation-editor"
          options={{ ...floatingHeader, title: "Automation" }}
        />
        <Stack.Screen
          name="automation/[id]"
          options={{ ...floatingHeader, title: "Run" }}
        />
        <Stack.Screen
          name="daemons"
          options={{ headerLargeTitle: true, title: "Daemons" }}
        />
        <Stack.Screen
          name="new-task"
          options={{ ...drawerHeader, title: "New Task" }}
        />
        <Stack.Screen
          name="session/[id]"
          options={{
            ...drawerHeader,
            animation: "none",
            headerTitleAlign: "left",
            title: "Task",
          }}
        />
      </Stack.Protected>
      <Stack.Screen
        name="daemon-editor"
        options={{
          presentation: "pageSheet",
          title: "Add Daemon",
        }}
      />
      <Stack.Screen name="gateway" options={{ title: "Personal gateway" }} />
      <Stack.Screen
        name="remote-executions"
        options={{ title: "Remote executions" }}
      />
      <Stack.Screen
        name="gateway-remote-executions"
        options={{ title: "Remote executions" }}
      />
      <Stack.Screen
        name="gateway-cloud-billing"
        options={{ title: "Cloud billing" }}
      />
      <Stack.Screen
        name="gateway-cloud-budgets"
        options={{ title: "GPU budgets" }}
      />
      <Stack.Screen
        name="gateway-workflow-history"
        options={{ title: "Workflow history" }}
      />
      <Stack.Screen
        name="gateway-workflow/[id]"
        options={{ title: "Workflow run" }}
      />
      <Stack.Screen
        name="gateway-workflow-evidence"
        options={{ title: "Workflow documents" }}
      />
      <Stack.Screen
        name="gateway-portfolio"
        options={{ title: "Gateway history" }}
      />
      <Stack.Screen
        name="gateway-automations"
        options={{ title: "Gateway workflows" }}
      />
      <Stack.Screen
        name="gateway-task/[id]"
        options={{ title: "Gateway task" }}
      />
    </Stack>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
});
