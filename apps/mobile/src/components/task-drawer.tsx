import type { AgentSession } from '@waku/client';
import * as Haptics from 'expo-haptics';
import { router, useGlobalSearchParams, usePathname } from 'expo-router';
import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import {
  ActivityIndicator,
  Alert,
  Animated,
  AppState,
  BackHandler,
  Easing,
  Keyboard,
  KeyboardAvoidingView,
  Pressable,
  RefreshControl,
  SectionList,
  StyleSheet,
  Text,
  TextInput,
  View,
  useWindowDimensions,
} from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { WorkbenchRow } from '@/components/workbench';
import { AppSymbol } from '@/components/app-symbol';
import { ConnectionBanner, useConnectionNotice } from '@/components/connection-banner';
import { DaemonPickerSheet } from '@/components/daemon-picker-sheet';
import { PressableScale } from '@/components/press-scale';
import { StatusPill, ticketStatus } from '@/components/ticket-card';
import { ConnectionStatus, connectionPhaseLabel } from '@/components/connection-status';
import { ProviderIcon } from '@/components/provider-icon';
import { RenameDialog } from '@/components/rename-dialog';
import { TaskRowMenu } from '@/components/task-row-menu';
import { MonoFont, Radius, Spacing } from '@/constants/theme';
import { useTaskState } from '@/hooks/use-daemon-data';
import { useTheme } from '@/hooks/use-theme';
import { useDaemon } from '@/lib/daemon-context';
import { sessionIsRunning } from '@/lib/mobile-runtime';
import { useRuntime } from '@/lib/runtime-context';
import { ticketNumber, ticketState } from '@/lib/ticket';
import {
  displaySessionTitle,
  groupSessions,
  providerLabel,
  relativeSessionTime,
  type SessionListItem,
} from '@/lib/session-presentation';

const DaemonPickerHeight = 38;
const SearchDockGap = 14;
/** Swipe thresholds, as react-native-drawer-layout had them. */
const SWIPE_MIN_OFFSET = 5;
const SWIPE_MIN_DISTANCE = 60;
const SWIPE_MIN_VELOCITY = 500;
interface TaskDrawerContextValue {
  openTaskDrawer: () => void;
  closeTaskDrawer: () => void;
}

const TaskDrawerContext = createContext<TaskDrawerContextValue | null>(null);

export function TaskDrawerHost({ children }: { children: ReactNode }) {
  const theme = useTheme();
  const daemon = useDaemon();
  const pathname = usePathname();
  const params = useGlobalSearchParams<{ id?: string | string[] }>();
  const { width } = useWindowDimensions();
  const [open, setOpen] = useState(false);
  const drawerWidth = Math.max(0, Math.min(360, width - 44));
  const drawerEnabled = daemon.phase === 'booting' || daemon.profiles.length > 0;
  const openTaskDrawer = useCallback(() => {
    if (drawerEnabled) setOpen(true);
  }, [drawerEnabled]);
  const closeTaskDrawer = useCallback(() => setOpen(false), []);
  const controls = useMemo(
    () => ({ openTaskDrawer, closeTaskDrawer }),
    [closeTaskDrawer, openTaskDrawer],
  );
  const swipeEnabled = pathname === '/'
    || pathname === '/new-task'
    || pathname.startsWith('/session/');
  const selectedSessionId = pathname.startsWith('/session/')
    ? Array.isArray(params.id) ? params.id[0] : params.id ?? null
    : null;

  useEffect(() => setOpen(false), [drawerEnabled, pathname]);

  // React state owns where the drawer rests. The previous Reanimated drawer
  // could lose its settled position when the JS thread was busy as it closed
  // (a route change, a slow device), and the next render redrew it open while
  // `open` was false, so the scrim, Back and swipes all did nothing.
  // Native-driven Animated syncs the final value back on completion instead.
  const progress = useRef(new Animated.Value(0)).current;
  const animateTo = useCallback((value: number) => {
    Animated.timing(progress, {
      duration: 240,
      easing: Easing.out(Easing.cubic),
      toValue: value,
      useNativeDriver: true,
    }).start();
  }, [progress]);
  useEffect(() => animateTo(open ? 1 : 0), [animateTo, open]);
  const pan = useMemo(() => Gesture.Pan()
    .runOnJS(true)
    .enabled(drawerEnabled && (swipeEnabled || open) && drawerWidth > 0)
    .activeOffsetX([-SWIPE_MIN_OFFSET, SWIPE_MIN_OFFSET])
    .failOffsetY([-SWIPE_MIN_OFFSET, SWIPE_MIN_OFFSET])
    .onStart(() => Keyboard.dismiss())
    .onUpdate((event) => {
      progress.setValue(Math.min(1, Math.max(0, (open ? 1 : 0) + event.translationX / drawerWidth)));
    })
    .onEnd((event) => {
      const swiped = Math.abs(event.translationX) > SWIPE_MIN_DISTANCE
        || (Math.abs(event.translationX) > SWIPE_MIN_OFFSET && Math.abs(event.velocityX) > SWIPE_MIN_VELOCITY);
      const next = swiped ? (event.velocityX === 0 ? event.translationX : event.velocityX) > 0 : open;
      if (next === open) animateTo(open ? 1 : 0);
      else setOpen(next);
    }), [animateTo, drawerEnabled, drawerWidth, open, progress, swipeEnabled]);

  // Back closes an open drawer before it can pop a screen or leave the app.
  useEffect(() => {
    if (!open) return;
    const subscription = BackHandler.addEventListener('hardwareBackPress', () => {
      setOpen(false);
      return true;
    });
    return () => {
      subscription.remove();
      // A focused search field must not keep its keyboard over the screen.
      Keyboard.dismiss();
    };
  }, [open]);

  return (
    <TaskDrawerContext.Provider value={controls}>
      {drawerEnabled ? (
        <GestureDetector gesture={pan}>
          <View collapsable={false} style={[styles.host, { backgroundColor: theme.background }]}>
            {/* Behind the screen, which slides aside to reveal it. */}
            <View style={[styles.drawer, { backgroundColor: theme.background, width: drawerWidth }]}>
              <TaskDrawerContent
                drawerWidth={drawerWidth}
                open={open}
                selectedSessionId={selectedSessionId}
                onClose={closeTaskDrawer}
              />
            </View>
            <Animated.View
              style={[styles.host, {
                backgroundColor: theme.background,
                transform: [{
                  translateX: progress.interpolate({ inputRange: [0, 1], outputRange: [0, drawerWidth] }),
                }],
              }]}>
              <View
                accessibilityElementsHidden={open}
                importantForAccessibility={open ? 'no-hide-descendants' : 'auto'}
                style={styles.host}>
                {children}
              </View>
              <Animated.View
                pointerEvents={open ? 'auto' : 'none'}
                style={[StyleSheet.absoluteFill, styles.scrim, { opacity: progress }]}>
                <Pressable
                  accessibilityLabel="Close task history"
                  accessibilityRole="button"
                  onPress={closeTaskDrawer}
                  style={StyleSheet.absoluteFill}
                />
              </Animated.View>
            </Animated.View>
          </View>
        </GestureDetector>
      ) : (
        <>{children}</>
      )}
    </TaskDrawerContext.Provider>
  );
}

export function useTaskDrawer(): TaskDrawerContextValue {
  const context = useContext(TaskDrawerContext);
  if (!context) throw new Error('useTaskDrawer must be used inside TaskDrawerHost');
  return context;
}

function TaskDrawerContent({
  drawerWidth,
  open,
  selectedSessionId,
  onClose,
}: {
  drawerWidth: number;
  open: boolean;
  selectedSessionId: string | null;
  onClose: () => void;
}) {
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const [search, setSearch] = useState('');
  const [refreshing, setRefreshing] = useState(false);
  const [daemonPickerOpen, setDaemonPickerOpen] = useState(false);
  const [renameTarget, setRenameTarget] = useState<AgentSession | null>(null);
  const [now, setNow] = useState(Date.now);
  // Share one clock across visible rows, and park it while the drawer or app
  // is hidden so recency labels add no work to the conversation screen.
  useEffect(() => {
    if (!open) return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const refresh = () => setNow(Date.now());
    const updateClock = (state: string | null) => {
      clearInterval(timer);
      if (state === 'active' || state === null) {
        refresh();
        timer = setInterval(refresh, 30_000);
      }
    };
    updateClock(AppState.currentState);
    const subscription = AppState.addEventListener('change', updateClock);
    return () => {
      clearInterval(timer);
      subscription.remove();
    };
  }, [open]);
  const listExtraData = useMemo(
    () => ({ runtimes: runtime.runtimes, permissions: runtime.permissions, userInputs: runtime.userInputs, now }),
    [runtime.runtimes, runtime.permissions, runtime.userInputs, now],
  );
  const visibleSessions = useMemo(() => {
    if (!taskState.data) return [];
    const query = search.trim().toLocaleLowerCase();
    if (!query) return taskState.data.sessions;
    const projects = new Map(taskState.data.projects.map((project) => [project.id, project]));
    return taskState.data.sessions.filter((session) => {
      const project = projects.get(session.project_id);
      return [
        displaySessionTitle(session),
        project?.name,
        project?.path,
        providerLabel(session.provider),
        session.model,
      ].some((value) => value?.toLocaleLowerCase().includes(query));
    });
  }, [search, taskState.data]);
  const sections = useMemo(
    () => taskState.data ? groupSessions(taskState.data.projects, visibleSessions) : [],
    [taskState.data, visibleSessions],
  );
  const showNewTask = useCallback(() => {
    onClose();
    router.dismissTo('/');
  }, [onClose]);
  const showSession = useCallback((sessionId: string) => {
    if (selectedSessionId === sessionId) {
      onClose();
    } else if (selectedSessionId) {
      router.setParams({ id: sessionId });
    } else {
      router.replace({ pathname: '/session/[id]', params: { id: sessionId } });
    }
  }, [onClose, selectedSessionId]);

  function confirmDelete(session: AgentSession) {
    Alert.alert(
      `Delete “${displaySessionTitle(session)}”?`,
      'This removes the task and its transcript from the daemon for every device.',
      [
        { text: 'Cancel', style: 'cancel' },
        {
          text: 'Delete',
          style: 'destructive',
          onPress: () => {
            void runtime.deleteSession(session.id)
              .then(() => Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success))
              .catch((cause) => {
                Alert.alert(
                  'Couldn’t delete task',
                  cause instanceof Error ? cause.message : String(cause),
                );
              });
          },
        },
      ],
    );
  }

  async function refreshTasks() {
    setRefreshing(true);
    try {
      if (daemon.phase === 'connected') await taskState.refetch();
      else await daemon.reconnect();
    } finally {
      setRefreshing(false);
    }
  }

  return (
    <View
      accessibilityElementsHidden={!open}
      importantForAccessibility={open ? 'auto' : 'no-hide-descendants'}
      pointerEvents={open ? 'auto' : 'none'}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      <View
        pointerEvents="box-none"
        style={[styles.daemonFloat, { top: insets.top + 8 }]}>
        <DaemonPill onPress={() => setDaemonPickerOpen(true)} />
      </View>

      <SectionList
        sections={sections}
        extraData={listExtraData}
        keyExtractor={(item) => item.session.id}
        contentContainerStyle={[
          styles.listContent,
          {
            paddingTop: insets.top + DaemonPickerHeight + 20,
          },
          sections.length === 0 && styles.listContentEmpty,
        ]}
        refreshControl={(
          <RefreshControl
            colors={[theme.textTertiary]}
            progressViewOffset={insets.top + DaemonPickerHeight + 12}
            refreshing={refreshing}
            tintColor={theme.textTertiary}
            onRefresh={() => void refreshTasks()}
          />
        )}
        renderSectionHeader={({ section }) => (
          <Text style={[styles.sectionTitle, { color: theme.textTertiary }]}>
            {section.title}
          </Text>
        )}
        renderItem={({ item }) => (
          <SessionRow
            drawerWidth={drawerWidth}
            animate={open}
            item={item}
            needsYou={Boolean(runtime.permissions[item.session.id] || runtime.userInputs[item.session.id]) || ticketState(item.session.status) === 'needsYou'}
            now={now}
            running={runtime.runtimes[item.session.id]?.running ?? sessionIsRunning(item.session)}
            selected={item.session.id === selectedSessionId}
            onDelete={() => confirmDelete(item.session)}
            onRename={() => setRenameTarget(item.session)}
            onSelect={() => showSession(item.session.id)}
          />
        )}
        ListHeaderComponent={<>
          <ConnectionBanner />
          <WorkbenchRow label="Work" detail="Tasks, projects, teams & decisions" icon={{ android: 'work_outline', web: 'work_outline' }} onPress={() => { onClose(); router.push('/work'); }} />
          <WorkbenchRow label="Artifacts" icon={{ android: 'description', web: 'description' }} onPress={() => { onClose(); router.push('/artifacts'); }} />
          <WorkbenchRow label="Settings" detail="Account, connections & usage" icon={{ android: 'settings', web: 'settings' }} onPress={() => { onClose(); router.push('/settings'); }} />
        </>}
        ListEmptyComponent={(
          <TaskListEmpty
            error={taskState.error}
            searching={Boolean(search.trim())}
            onNewTask={showNewTask}
          />
        )}
        showsVerticalScrollIndicator={false}
        stickySectionHeadersEnabled={false}
      />

      {(daemon.profiles.length > 0 || daemon.phase === 'booting') && (
        <KeyboardAvoidingView
          // Edge-to-edge Android no longer resizes the window for the keyboard.
          behavior="position"
          keyboardVerticalOffset={SearchDockGap}
          pointerEvents="box-none"
          style={[styles.searchDockAvoider, { bottom: insets.bottom + SearchDockGap }]}>
          <View pointerEvents="box-none" style={styles.searchDock}>
            <View style={[styles.searchCapsule, { backgroundColor: theme.ticket, borderColor: theme.borderStrong, boxShadow: theme.elevationFloat }]}>
              <View style={styles.searchCapsuleInner}>
                <AppSymbol
                  name={{ ios: 'magnifyingglass', android: 'search', web: 'search' }}
                  size={17}
                  tintColor={theme.textSecondary}
                />
                <TextInput
                  accessibilityLabel="Search tasks"
                  autoCapitalize="none"
                  autoCorrect={false}
                  placeholder="Search"
                  placeholderTextColor={theme.textTertiary}
                  selectionColor={theme.accent}
                  style={[styles.searchInput, { color: theme.text }]}
                  value={search}
                  onChangeText={setSearch}
                />
                {search.length > 0 && (
                  <Pressable
                    accessibilityLabel="Clear search"
                    accessibilityRole="button"
                    onPress={() => setSearch('')}
                    style={({ pressed }) => [styles.clearButton, { opacity: pressed ? 0.5 : 1 }]}>
                    <AppSymbol
                      name={{ ios: 'xmark.circle.fill', android: 'cancel', web: 'cancel' }}
                      size={16}
                      tintColor={theme.textTertiary}
                    />
                  </Pressable>
                )}
              </View>
            </View>
            {daemon.phase === 'connected' && (
              <PressableScale
                accessibilityLabel="New task"
                accessibilityRole="button"
                onPress={showNewTask}
                style={({ pressed }) => [styles.composeButton, {
                  backgroundColor: theme.primary,
                  boxShadow: theme.elevationFloat,
                  opacity: pressed ? 0.85 : 1,
                }]}>
                <AppSymbol
                  name={{ ios: 'square.and.pencil', android: 'edit_square', web: 'edit' }}
                  size={20}
                  tintColor={theme.onPrimary}
                />
              </PressableScale>
            )}
          </View>
        </KeyboardAvoidingView>
      )}

      {renameTarget && (
        <RenameDialog
          initialValue={displaySessionTitle(renameTarget)}
          onDismiss={() => setRenameTarget(null)}
          onSubmit={(title) => runtime.renameSession(renameTarget.id, title)}
          visible
        />
      )}
      <DaemonPickerSheet
        onDismiss={() => setDaemonPickerOpen(false)}
        visible={daemonPickerOpen}
      />
    </View>
  );
}

function DaemonPill({ onPress }: { onPress: () => void }) {
  const theme = useTheme();
  const daemon = useDaemon();
  return (
    <View style={[styles.daemonButton, { backgroundColor: theme.ticket, borderColor: theme.ticketBorder, boxShadow: theme.elevationRest }]}>
      <Pressable
        accessibilityHint="Opens the daemon switcher"
        accessibilityLabel={daemon.activeProfile
          ? `${connectionPhaseLabel(daemon.phase)}: ${daemon.activeProfile.name}`
          : 'Add a daemon'}
        accessibilityRole="button"
        hitSlop={8}
        onPress={onPress}
        style={({ pressed }) => [styles.daemonButtonInner, { backgroundColor: pressed ? theme.overlay : 'transparent' }]}>
        {daemon.activeProfile ? <ConnectionStatus compact phase={daemon.phase} /> : (
          <AppSymbol
            name={{ ios: 'plus', android: 'add', web: 'add' }}
            size={14}
            tintColor={theme.text}
          />
        )}
        <Text numberOfLines={1} style={[styles.daemonButtonText, { color: theme.text }]}>
          {daemon.activeProfile?.name ?? 'Add daemon'}
        </Text>
        <AppSymbol
          name={{ ios: 'chevron.down', android: 'keyboard_arrow_down', web: 'keyboard_arrow_down' }}
          size={12}
          tintColor={theme.textTertiary}
        />
      </Pressable>
    </View>
  );
}

function TaskListEmpty({
  error,
  searching,
  onNewTask,
}: {
  error: unknown;
  searching: boolean;
  onNewTask: () => void;
}) {
  const theme = useTheme();
  const { phase } = useDaemon();
  const notice = useConnectionNotice();
  // The banner above the list is already explaining the wait.
  if (notice && notice.kind !== 'restored') return null;
  if (phase === 'booting' || phase === 'connecting' || phase === 'reconnecting') {
    return (
      <View style={styles.emptyState}>
        <ActivityIndicator color={theme.textTertiary} />
        <Text style={[styles.emptyTitle, { color: theme.textSecondary }]}>
          {phase === 'reconnecting' ? 'Reconnecting…' : 'Connecting to daemon…'}
        </Text>
      </View>
    );
  }
  if (searching) {
    return (
      <View style={styles.emptyState}>
        <Text style={[styles.emptyTitle, { color: theme.text }]}>No matching tasks</Text>
        <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>Try another title, project, or agent.</Text>
      </View>
    );
  }
  if (error) {
    return (
      <View style={styles.emptyState}>
        <Text style={[styles.emptyTitle, { color: theme.text }]}>Couldn’t load tasks</Text>
        <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
          {error instanceof Error ? error.message : String(error)}
        </Text>
      </View>
    );
  }
  return (
    <View style={styles.emptyState}>
      <View style={[styles.emptyIcon, { backgroundColor: theme.accentSoft }]}>
        <AppSymbol
          name={{ ios: 'text.bubble', android: 'chat_bubble', web: 'chat' }}
          size={25}
          tintColor={theme.accent}
        />
      </View>
      <Text style={[styles.emptyTitle, { color: theme.text }]}>No tasks yet</Text>
      <Text style={[styles.emptyBody, { color: theme.textSecondary }]}>
        Start an agent on anything: a bug, a feature, a question about the code.
      </Text>
      {phase === 'connected' && (
        <PressableScale
          accessibilityRole="button"
          onPress={onNewTask}
          style={({ pressed }) => [
            styles.emptyAction,
            { backgroundColor: theme.primary, opacity: pressed ? 0.85 : 1 },
          ]}>
          <Text style={[styles.emptyActionText, { color: theme.onPrimary }]}>New task</Text>
        </PressableScale>
      )}
    </View>
  );
}

function SessionRow({
  animate,
  drawerWidth,
  item,
  needsYou,
  now,
  running,
  selected,
  onDelete,
  onRename,
  onSelect,
}: {
  /** Spinners only run while the drawer is open: a closed drawer stays
   * mounted, and a native spinner would otherwise redraw every frame. */
  animate: boolean;
  drawerWidth: number;
  item: SessionListItem;
  needsYou: boolean;
  now: number;
  running: boolean;
  selected: boolean;
  onDelete: () => void;
  onRename: () => void;
  onSelect: () => void;
}) {
  const theme = useTheme();
  const rowWidth = Math.max(0, drawerWidth - 24);
  const session = item.session;
  const lastReplyTime = session.last_reply_at == null
    ? null
    : relativeSessionTime(session.last_reply_at, now);
  return (
    <TaskRowMenu
      accessibilityLabel={`${displaySessionTitle(session)}, ${providerLabel(session.provider)} in ${item.projectName}${needsYou ? ', Needs you' : running ? ', Running' : ''}${lastReplyTime ? `, Last reply: ${lastReplyTime}` : ''}`}
      onDelete={onDelete}
      onRename={onRename}
      onSelect={onSelect}
      renderTrigger={(pressed) => (
        <View
          collapsable={false}
          style={[
            styles.sessionRow,
            {
              backgroundColor: pressed
                ? theme.overlayStrong
                : selected ? theme.ticket : 'transparent',
              borderColor: needsYou ? theme.primary : selected ? theme.ticketBorder : 'transparent',
              boxShadow: selected ? theme.elevationRest : undefined,
              width: rowWidth,
            },
          ]}>
          <View style={styles.sessionHeading}>
            <Text numberOfLines={1} style={[styles.sessionTitle, { color: theme.text }]}>
              {displaySessionTitle(session)}
            </Text>
            {needsYou ? <StatusPill {...ticketStatus('needsYou')} /> : running && animate && (
              <ActivityIndicator
                accessibilityLabel="Running"
                color={theme.accent}
                size="small"
                style={styles.sessionSpinner}
              />
            )}
          </View>
          <View style={styles.sessionMetadata}>
            <Text style={[styles.sessionNumber, { color: needsYou ? theme.accent : theme.textTertiary }]}>
              {ticketNumber(session.id)}
            </Text>
            <ProviderIcon color={theme.textTertiary} provider={session.provider} size={12} />
            <Text
              numberOfLines={1}
              style={[styles.sessionProject, { color: theme.textTertiary }]}>
              {item.projectName}
            </Text>
            {lastReplyTime !== null && (
              <Text
                numberOfLines={1}
                style={[styles.sessionTime, { color: theme.textTertiary }]}>
                {lastReplyTime}
              </Text>
            )}
          </View>
        </View>
      )}
      selected={selected}
      style={[styles.sessionMenu, { width: rowWidth }]}
    />
  );
}

const styles = StyleSheet.create({
  host: { flex: 1 },
  drawer: { bottom: 0, left: 0, position: 'absolute', top: 0 },
  scrim: { backgroundColor: 'rgba(0, 0, 0, 0.18)' },
  screen: { flex: 1 },
  daemonFloat: {
    left: 12,
    position: 'absolute',
    zIndex: 30,
  },
  searchDockAvoider: {
    left: 0,
    position: 'absolute',
    right: 0,
    zIndex: 20,
  },
  searchDock: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 10,
    paddingHorizontal: Spacing.three,
  },
  searchCapsule: { borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, flex: 1 },
  searchCapsuleInner: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 9,
    minHeight: 50,
    paddingLeft: 16,
    paddingRight: 4,
  },
  clearButton: { alignItems: 'center', height: 48, justifyContent: 'center', width: 44 },
  searchInput: { flex: 1, fontSize: 16.5, paddingVertical: 10 },
  composeButton: { alignItems: 'center', borderRadius: Radius.pill, height: 50, justifyContent: 'center', width: 50 },
  daemonButton: { borderRadius: Radius.pill, borderWidth: StyleSheet.hairlineWidth, maxWidth: 196, overflow: 'hidden' },
  daemonButtonInner: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: 6,
    minHeight: DaemonPickerHeight,
    paddingHorizontal: 12,
  },
  daemonButtonText: { flexShrink: 1, fontSize: 13, fontWeight: '600' },
  listContent: { paddingBottom: 96 },
  listContentEmpty: { flexGrow: 1 },
  sectionTitle: {
    fontSize: 13,
    fontWeight: '500',
    letterSpacing: 0,
    marginBottom: 6,
    marginHorizontal: 24,
    marginTop: 18,
  },
  emptyState: {
    alignItems: 'center',
    flex: 1,
    justifyContent: 'center',
    minHeight: 360,
    paddingHorizontal: 40,
  },
  emptyIcon: {
    alignItems: 'center',
    borderRadius: 16,
    height: 64,
    justifyContent: 'center',
    marginBottom: 18,
    width: 64,
  },
  emptyTitle: { fontSize: 17, fontWeight: '700', textAlign: 'center' },
  emptyBody: { fontSize: 14, lineHeight: 20, marginTop: 7, maxWidth: 320, textAlign: 'center' },
  emptyAction: {
    borderRadius: Radius.pill,
    justifyContent: 'center',
    marginTop: 18,
    minHeight: 48,
    paddingHorizontal: 22,
  },
  emptyActionText: { fontSize: 14, fontWeight: '700' },
  sessionMenu: { height: 64, marginHorizontal: 12 },
  sessionRow: {
    borderRadius: Radius.medium,
    borderWidth: 1,
    gap: 4,
    height: 64,
    justifyContent: 'center',
    paddingHorizontal: 12,
  },
  sessionHeading: { alignItems: 'center', flexDirection: 'row', gap: 8 },
  sessionMetadata: { alignItems: 'center', flexDirection: 'row', gap: 6 },
  sessionNumber: { fontFamily: MonoFont, fontSize: 12, fontVariant: ['tabular-nums'], lineHeight: 17 },
  sessionProject: { flex: 1, fontSize: 12.5, lineHeight: 17 },
  sessionTime: { flexShrink: 0, fontSize: 12.5, lineHeight: 17, marginLeft: 3 },
  sessionSpinner: { height: 14, transform: [{ scale: 0.72 }], width: 14 },
  sessionTitle: {
    flex: 1,
    fontSize: 15.5,
    fontWeight: '500',
    lineHeight: 21,
  },
});
