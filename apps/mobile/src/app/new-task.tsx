import { useQuery } from '@tanstack/react-query';
import type { ProviderKind, RuntimeMode, Team } from '@waku/client';
import { teamReference } from '@waku/client/composer-autocomplete';
import {
  rememberedModelTraits,
  rememberComposerSession,
  type ComposerPreferences,
} from '@waku/client/composer-preferences';
import * as Haptics from 'expo-haptics';
import { router, useLocalSearchParams } from 'expo-router';
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  ActivityIndicator,
  KeyboardAvoidingView,
  Platform,
  ScrollView,
  StyleSheet,
  Text,
  View,
  useWindowDimensions,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { AppSymbol } from '@/components/app-symbol';
import { Chip } from '@/components/chip';
import { ConnectionBanner } from '@/components/connection-banner';
import { ConnectionStatus } from '@/components/connection-status';
import { GlowField } from '@/components/glow-field';
import { LiveWork } from '@/components/live-work';
import { ProviderIcon } from '@/components/provider-icon';
import { ComposerAccessMenu } from '@/components/composer-access-menu';
import { ComposerAddMenu } from '@/components/composer-add-menu';
import { ComposerContextPicker } from '@/components/composer-context-picker';
import { useComposerLocalCommands } from '@/components/composer-command-sheets';
import { DaemonPickerSheet } from '@/components/daemon-picker-sheet';
import { PhoneDictationControl } from '@/components/phone-dictation-control';
import { LiveDictationPreview } from '@/components/live-dictation-preview';
import {
  ComposerCard,
  ComposerIconButton,
  SendButton,
} from '@/components/mobile-composer';
import { RemoteProjectPicker } from '@/components/remote-project-picker';
import { useScreenHeaderInset } from '@/components/screen-header';
import {
  ModelPickerSheet,
  ModelTraitsSheet,
  type ProviderModelSelection,
} from '@/components/session-option-sheets';
import { Sheet, SheetRow } from '@/components/sheet';
import { MonoFont, Radius } from '@/constants/theme';
import { useAllProviderModels, useProviderCatalog, useTaskState } from '@/hooks/use-daemon-data';
import { useComposerPicker } from '@/hooks/use-composer-picker';
import { useSyncedComposerDraft } from '@/hooks/use-synced-composer-draft';
import { useTheme } from '@/hooks/use-theme';
import { daemonKeys, inspectBranches } from '@/lib/daemon-api';
import { composerProviderPrompt } from '@/lib/composer-completion';
import {
  loadComposerPreferences,
  loadNewTaskExtras,
  saveComposerPreferences,
  saveNewTaskExtras,
} from '@/lib/composer-preferences-store';
import { useDaemon } from '@/lib/daemon-context';
import {
  modelHasConfigurableTraits,
  type ModelTraitSelection,
} from '@/lib/model-traits';
import { useRuntime } from '@/lib/runtime-context';
import { providerLabel } from '@/lib/session-presentation';
import { mobileHarness } from '@/lib/mobile-capabilities';

type SheetKind =
  | 'daemon'
  | 'project'
  | 'model'
  | 'traits'
  | 'workspace'
  | 'branch';

export default function NewTaskScreen() {
  const proposalParams = useLocalSearchParams<{proposal?:string;owner?:string;project?:string;team?:string}>();
  const teamLoaded = useRef<string | null>(null);
  const proposalLoaded = useRef<string | null>(null);
  const theme = useTheme();
  const insets = useSafeAreaInsets();
  const headerInset = useScreenHeaderInset();
  const { height } = useWindowDimensions();
  const daemon = useDaemon();
  const runtime = useRuntime();
  const taskState = useTaskState();
  const catalog = useProviderCatalog();
  const [projectId, setProjectId] = useState<string | null>(null);
  const [provider, setProvider] = useState<ProviderKind | null>(null);
  const [model, setModel] = useState<string | null>(null);
  const [reasoningEffort, setReasoningEffort] = useState<string | null>(null);
  const [serviceTier, setServiceTier] = useState<string | null>(null);
  const [contextWindow, setContextWindow] = useState<string | null>(null);
  const [runtimeMode, setRuntimeMode] = useState<RuntimeMode>('fullAccess');
  const [isolated, setIsolated] = useState(false);
  const [baseBranch, setBaseBranch] = useState<string | null>(null);
  const [prompt, setPrompt] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [openSheet, setOpenSheet] = useState<SheetKind | null>(null);
  const [projectPickerOpen, setProjectPickerOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [speechPreview, setSpeechPreview] = useState<string | null>(null);
  const installedProviders = useMemo(
    () => catalog.providers.filter((item) => item.installed).map((item) => item.id),
    [catalog.providers],
  );
  const modelCatalog = useAllProviderModels(installedProviders);
  const projects = taskState.data?.projects ?? [];
  const selectedProject = projects.find((item) => item.id === projectId);
  const projectless = selectedProject?.name === 'No project';
  const branches = useQuery({
    queryKey: daemonKeys.branches(
      daemon.activeProfile?.id ?? 'disconnected',
      selectedProject?.path ?? 'missing',
    ),
    queryFn: () => inspectBranches(daemon.client!, selectedProject!.path),
    enabled: daemon.phase === 'connected' && Boolean(daemon.client && selectedProject) && isolated,
    staleTime: 30_000,
  });

  useEffect(() => {
    if (!projectId || !projects.some((project) => project.id === projectId)) {
      setProjectId(projects[0]?.id ?? null);
    }
  }, [projectId, projects]);

  useEffect(() => {
    if (projectless && isolated) setIsolated(false);
  }, [isolated, projectless]);

  useEffect(() => setBaseBranch(null), [projectId]);

  useEffect(() => {
    if (provider && installedProviders.includes(provider)) return;
    const preferred = installedProviders[0];
    if (preferred) {
      setProvider(preferred);
      setModel(null);
      setReasoningEffort(null);
      setServiceTier(null);
      setContextWindow(null);
    }
  }, [installedProviders, provider]);

  // Restore the last-used composition (provider/model traits via the shared
  // composer preferences, plus mobile extras) once per daemon.
  const [restoredAddress, setRestoredAddress] = useState<string | null>(null);
  const restoredFor = useRef<string | null>(null);
  const preferencesRef = useRef<ComposerPreferences | null>(null);
  useEffect(() => {
    const address = daemon.activeProfile?.address;
    if (!address || restoredFor.current === address) return;
    restoredFor.current = address;
    preferencesRef.current = null;
    setRestoredAddress(null);
    void (async () => {
      const [prefs, extras] = await Promise.all([
        loadComposerPreferences(address),
        loadNewTaskExtras(address),
      ]);
      setRuntimeMode(extras.runtimeMode);
      setIsolated(extras.isolated);
      if (extras.projectId) setProjectId(extras.projectId);
      setProvider(mobileHarness(prefs.lastProvider) ? prefs.lastProvider : null);
      setModel(mobileHarness(prefs.lastProvider) ? prefs.lastModel : null);
      setReasoningEffort(mobileHarness(prefs.lastProvider) ? prefs.lastReasoningEffort : null);
      setServiceTier(mobileHarness(prefs.lastProvider) ? prefs.lastServiceTier : null);
      setContextWindow(mobileHarness(prefs.lastProvider) ? prefs.lastContextWindow : null);
      preferencesRef.current = prefs;
      setRestoredAddress(address);
    })();
  }, [daemon.activeProfile?.address]);

  // Unlike the web SPA, this screen unmounts between visits, so every choice
  // persists as it is made — not only when a task is created.
  useEffect(() => {
    const address = daemon.activeProfile?.address;
    if (!address || restoredAddress !== address) return;
    const timer = setTimeout(() => {
      void loadComposerPreferences(address).then((stored) => {
        const prefs = preferencesRef.current ?? stored;
        let next: ComposerPreferences = {
          ...prefs,
          ...(provider ? { lastProvider: provider } : {}),
          lastModel: model,
          lastReasoningEffort: reasoningEffort,
          lastServiceTier: serviceTier,
          lastContextWindow: contextWindow,
        };
        if (provider && model) {
          next = rememberComposerSession(next, {
            provider,
            model,
            reasoning_effort: reasoningEffort,
            service_tier: serviceTier,
            context_window: contextWindow,
          });
        }
        preferencesRef.current = next;
        return saveComposerPreferences(address, next);
      }).catch(() => {});
      void saveNewTaskExtras(address, {
        runtimeMode,
        isolated,
        projectId,
      }).catch(() => {});
    }, 300);
    return () => clearTimeout(timer);
  }, [
    contextWindow,
    daemon.activeProfile?.address,
    isolated,
    model,
    projectId,
    provider,
    reasoningEffort,
    restoredAddress,
    runtimeMode,
    serviceTier,
  ]);

  useEffect(() => {
    if (proposalParams.project && taskState.data?.projects.some(project => project.id === proposalParams.project)) setProjectId(proposalParams.project);
  }, [proposalParams.project, taskState.data?.projects]);

  // Cross-device draft: hydrate again whenever this surface becomes active,
  // but persist only real local edits. Echoing a hydrated value would let a
  // backgrounded mobile client resurrect it after desktop submits it.
  const draftSync = useSyncedComposerDraft({
    target: selectedProject
      ? { type: 'newSession', projectId: selectedProject.id }
      : null,
    text: prompt,
    carryAcrossTargets: true,
    onHydrate: (synchronized) => setPrompt(synchronized.text),
  });
  // An accepted proposal only fills the existing composer. No provider starts here.
  const draftSyncRef=useRef(draftSync);draftSyncRef.current=draftSync;
  const promptRef=useRef(prompt);promptRef.current=prompt;
  useEffect(()=> {
    const client=daemon.client; const id=proposalParams.proposal;
    const owner=daemon.activeProfile?.id; const key=`${owner}:${id}`;
    if(!id || !client || daemon.phase!=='connected' || proposalParams.owner!==owner || restoredAddress!==daemon.activeProfile?.address || proposalLoaded.current===key)return;
    let canceled=false;
    void client.request({type:'loadWorkspaceControl'}).then((response)=> {
      if(canceled)return;
      if(response.type!=='workspaceControl')throw new Error('Could not load the accepted proposal.');
      const action=response.snapshot.records.find((r)=>r.kind==='action' && r.id===id);
      if(!action || action.kind!=='action' || action.status!=='accepted' || action.actionKind!=='proposedTask')throw new Error('This task proposal is no longer accepted or available.');
      if(!taskState.data?.projects.some((p)=>p.id===action.projectId))throw new Error('The proposal’s project is unavailable.');
      if(promptRef.current.trim())throw new Error('Clear or save the current draft, then reopen the accepted proposal.');
      proposalLoaded.current=key;
      draftSyncRef.current.markEdited();setProjectId(action.projectId);setPrompt(`${action.title}\n\n${action.detail}`);
    }).catch((cause)=>{if(!canceled)setError(cause instanceof Error?cause.message:String(cause));});
    return()=>{canceled=true;};
  },[daemon.client,daemon.phase,daemon.activeProfile?.id,daemon.activeProfile?.address,proposalParams.proposal,proposalParams.owner,restoredAddress,taskState.data]);
  // A team's "New task" opens here with the team addressed; the user writes the objective.
  useEffect(() => {
    const client = daemon.client; const id = proposalParams.team;
    const key = `${daemon.activeProfile?.id}:${id}`;
    if (!id || !client || daemon.phase !== 'connected' || restoredAddress !== daemon.activeProfile?.address || teamLoaded.current === key) return;
    let canceled = false;
    void client.request({ type: 'loadWorkspaceControl' }).then((response) => {
      if (canceled) return;
      if (response.type !== 'workspaceControl') throw new Error('Could not load the team.');
      const teams = response.snapshot.records.filter((r): r is Team & { kind: 'team' } => r.kind === 'team');
      const team = teams.find((t) => t.id === id);
      if (!team) throw new Error('This team is no longer available.');
      teamLoaded.current = key;
      const mention = teamReference(team, teams);
      if (promptRef.current.startsWith(mention)) return;
      draftSyncRef.current.markEdited();
      setPrompt(`${mention} ${promptRef.current.trimStart()}`);
    }).catch((cause) => { if (!canceled) setError(cause instanceof Error ? cause.message : String(cause)); });
    return () => { canceled = true; };
  }, [daemon.client, daemon.phase, daemon.activeProfile?.id, daemon.activeProfile?.address, proposalParams.team, restoredAddress]);
  const contextPicker = useComposerPicker({
    text: prompt,
    onChangeText: (value) => {
      draftSync.markEdited();
      setPrompt(value);
    },
    provider,
    root: selectedProject?.path ?? null,
    contextKey: selectedProject?.id ?? 'new-task',
  });

  function pick(apply: () => void) {
    return () => {
      void Haptics.selectionAsync();
      apply();
      setOpenSheet(null);
    };
  }

  function applyModelSelection(selection: ProviderModelSelection) {
    let preferences = preferencesRef.current;
    if (preferences && provider && model) {
      preferences = rememberComposerSession(preferences, {
        provider,
        model,
        reasoning_effort: reasoningEffort,
        service_tier: serviceTier,
        context_window: contextWindow,
      });
      preferencesRef.current = preferences;
    }
    const remembered = preferences && selection.model
      ? rememberedModelTraits(preferences, selection.provider, selection.model)
      : undefined;
    setProvider(selection.provider);
    setModel(selection.model);
    setReasoningEffort(remembered ? remembered.reasoningEffort : selection.reasoningEffort);
    setServiceTier(remembered ? remembered.serviceTier : selection.serviceTier);
    setContextWindow(remembered ? remembered.contextWindow : selection.contextWindow);
  }

  function applyModelTraits(changes: Partial<ModelTraitSelection>) {
    if (changes.reasoningEffort !== undefined) setReasoningEffort(changes.reasoningEffort);
    if (changes.serviceTier !== undefined) setServiceTier(changes.serviceTier);
    if (changes.contextWindow !== undefined) setContextWindow(changes.contextWindow);
  }

  async function start() {
    const value = prompt.trim();
    if (!selectedProject || !provider || !value || submitting) return;
    setSubmitting(true);
    setError(null);
    try {
      const commands = value.startsWith('/') ? await contextPicker.getCommands() : [];
      if (await localCommands.execute(value, commands)) {
        setSubmitting(false);
        return;
      }
      const session = await runtime.createTask(
        selectedProject.id,
        provider,
        isolated && !projectless,
        value,
        {
          model,
          reasoningEffort,
          serviceTier,
          contextWindow,
          runtimeMode,
          baseBranch,
        },
        composerProviderPrompt(provider, value, commands),
      );
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
      const address = daemon.activeProfile?.address;
      if (address) {
        void loadComposerPreferences(address).then((stored) => {
          const prefs = preferencesRef.current ?? stored;
          let next = rememberComposerSession(prefs, session);
          if (!session.model) {
            next = {
              ...next,
              lastProvider: session.provider,
              lastModel: null,
              lastReasoningEffort: session.reasoning_effort ?? null,
              lastServiceTier: session.service_tier ?? null,
              lastContextWindow: session.context_window ?? null,
            };
          }
          preferencesRef.current = next;
          return saveComposerPreferences(address, next);
        }).catch(() => {});
        void saveNewTaskExtras(address, {
          runtimeMode,
          isolated: isolated && !projectless,
          projectId: selectedProject.id,
        }).catch(() => {});
      }
      draftSync.removeSubmittedDraft();
      setPrompt('');
      setSubmitting(false);
      router.push({ pathname: '/session/[id]', params: { id: session.id } });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
      await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
      setSubmitting(false);
    }
  }

  const providerModels = modelCatalog.find((entry) => entry.id === provider)?.models ?? [];
  const activeModel = model
    ? providerModels.find((item) => item.id === model)
    : providerModels.find((item) => item.is_default) ?? providerModels[0];
  const localCommands = useComposerLocalCommands({
    provider,
    model: activeModel,
    serviceTier,
    runtimeMode,
    contextKey: selectedProject?.id ?? 'new-task',
    onServiceTier: setServiceTier,
    onGoal: async (operation) => {
      if (!selectedProject || !provider) throw new Error('Choose a project and model first');
      const session = await runtime.createTask(selectedProject.id, provider, isolated && !projectless, '', {
        model, reasoningEffort, serviceTier, contextWindow, runtimeMode, baseBranch,
      });
      await runtime.sendGoalOperation(session, operation);
      router.push({ pathname: '/session/[id]', params: { id: session.id } });
    },
    onClear: () => {
      draftSync.markEdited();
      setPrompt('');
    },
  });
  const modelLabel = !provider
    ? catalog.isPending ? 'Checking agents…' : 'No agents installed'
    : activeModel?.name ?? model ?? providerLabel(provider);
  const branchLabel = baseBranch
    ?? branches.data?.default_branch
    ?? branches.data?.current
    ?? 'Default branch';
  const startDisabled = !selectedProject || !provider || !prompt.trim() || submitting;

  const connecting = daemon.phase === 'connecting' || daemon.phase === 'booting' || daemon.phase === 'reconnecting';

  return (
    <KeyboardAvoidingView
      behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
      style={[styles.screen, { backgroundColor: theme.background }]}>
      <GlowField />
      <ScrollView
        keyboardShouldPersistTaps="handled"
        contentContainerStyle={[styles.home, {
          paddingTop: headerInset + Math.max(16, Math.round(height * 0.05)),
          paddingBottom: Math.max(insets.bottom, 16) + 16,
        }]}>
        {/* Title and menu button are the native navigation bar's; the headline sits clear of it. */}
        <Text accessibilityRole="header" style={[styles.headline, { color: theme.text }]}>
          What should we build?
        </Text>
        <ConnectionBanner style={styles.banner} />
        <View style={styles.composer}>
          {error && (
            <View
              accessibilityLiveRegion="polite"
              style={[styles.error, { backgroundColor: theme.dangerSoft }]}>
              <Text style={[styles.errorText, { color: theme.danger }]}>{error}</Text>
            </View>
          )}
          <LiveDictationPreview text={speechPreview} />
          <ComposerCard
            {...contextPicker.inputProps}
            accessibilityLabel="Task prompt"
            editable={!submitting}
            left={(
              <>
                <ComposerAddMenu
                  disabled={submitting || daemon.phase !== 'connected' || !selectedProject || !provider}
                  onChooseContext={contextPicker.open}
                />
                <Chip
                  accessibilityLabel={`Choose model: ${modelLabel}`}
                  icon={provider ? undefined : { ios: 'sparkle', android: 'auto_awesome', web: 'auto_awesome' }}
                  label={modelLabel}
                  mark={provider ? <ProviderIcon provider={provider} size={16} /> : undefined}
                  maxLabelWidth={120}
                  onPress={() => setOpenSheet('model')}
                />
                {activeModel && modelHasConfigurableTraits(activeModel) && (
                  <ComposerIconButton
                    icon={{ ios: 'speedometer', android: 'speed', web: 'speed' }}
                    label="Model options"
                    onPress={() => setOpenSheet('traits')}
                  />
                )}
              </>
            )}
            placeholder="Do anything…"
            right={(
              <>
                <PhoneDictationControl
                  client={daemon.client}
                  onPreview={setSpeechPreview}
                  contextKey={`new-task:${projectId ?? ''}:${provider ?? ''}`}
                  host={daemon.activeProfile?.name ?? 'the selected daemon'}
                  disabled={submitting || daemon.phase !== 'connected'}
                  onError={setError}
                  onTranscript={(text) => {
                    draftSync.markEdited();
                    setPrompt((current) => current.trim() ? `${current.trimEnd()} ${text}` : text);
                  }}
                />
                <SendButton
                  busy={submitting}
                  disabled={startDisabled}
                  label="Start task"
                  onPress={() => void start()}
                />
              </>
            )}
            value={prompt}
          />
          {/* One scrolling line: wrapping stranded the last chip on a row of its own. */}
          <ScrollView
            horizontal
            keyboardShouldPersistTaps="handled"
            showsHorizontalScrollIndicator={false}
            style={styles.contextScroller}
            contentContainerStyle={styles.context}>
            <Chip
              quiet
              accessibilityLabel={`Project: ${selectedProject?.name ?? 'Choose a project'}`}
              disabled={taskState.isPending}
              icon={{ ios: 'folder', android: 'folder_open', web: 'folder_open' }}
              label={taskState.isPending
                ? daemon.phase === 'connected' ? 'Loading projects…' : 'Project'
                : selectedProject?.name ?? 'Choose project'}
              maxLabelWidth={160}
              onPress={() => setOpenSheet('project')}
            />
            <Chip
              quiet
              accessibilityLabel={`Desktop: ${daemon.activeProfile?.name ?? 'Pair desktop'}`}
              label={connecting ? 'Connecting…' : daemon.activeProfile?.name ?? 'Pair desktop'}
              mark={daemon.activeProfile ? <ConnectionStatus compact phase={daemon.phase} /> : undefined}
              icon={daemon.activeProfile ? undefined : { ios: 'laptopcomputer', android: 'laptop_mac', web: 'laptop_mac' }}
              maxLabelWidth={140}
              onPress={() => setOpenSheet('daemon')}
            />
            <Chip
              quiet
              accessibilityLabel={`Workspace: ${isolated ? 'Isolated worktree' : 'Work locally'}`}
              icon={{ ios: 'arrow.triangle.branch', android: 'account_tree', web: 'account_tree' }}
              label={isolated ? 'Worktree' : 'Local'}
              onPress={() => setOpenSheet('workspace')}
            />
            {isolated && (
              <Chip
                quiet
                accessibilityLabel={`Base branch: ${branchLabel}`}
                disabled={branches.isPending && branches.fetchStatus !== 'idle'}
                icon={{ ios: 'arrow.branch', android: 'call_split', web: 'call_split' }}
                label={branchLabel}
                maxLabelWidth={140}
                onPress={() => setOpenSheet('branch')}
              />
            )}
            <ComposerAccessMenu quiet mode={runtimeMode} onApply={setRuntimeMode} />
          </ScrollView>
        </View>
        <LiveWork />
      </ScrollView>

      <DaemonPickerSheet
        onDismiss={() => setOpenSheet(null)}
        visible={openSheet === 'daemon'}
      />

      <Sheet onDismiss={() => setOpenSheet(null)} title="Project" visible={openSheet === 'project'}>
        {projects.map((project) => (
          <SheetRow
            description={project.path}
            key={project.id}
            label={project.name}
            onPress={pick(() => setProjectId(project.id))}
            selected={project.id === projectId}
          />
        ))}
        <SheetRow
          description="Pick any folder on the daemon host, or create an empty workspace"
          label="Browse daemon host…"
          leading={(
            <AppSymbol
              name={{ ios: 'externaldrive', android: 'hard_drive', web: 'hard_drive' }}
              size={18}
              tintColor={theme.textSecondary}
            />
          )}
          onPress={() => {
            setOpenSheet(null);
            setProjectPickerOpen(true);
          }}
        />
      </Sheet>

      <ModelPickerSheet
        model={model}
        onApply={applyModelSelection}
        onDismiss={() => setOpenSheet(null)}
        provider={provider}
        providers={installedProviders}
        visible={openSheet === 'model'}
      />

      {activeModel && (
        <ModelTraitsSheet
          model={activeModel}
          onApply={applyModelTraits}
          onDismiss={() => setOpenSheet(null)}
          selection={{ reasoningEffort, serviceTier, contextWindow }}
          visible={openSheet === 'traits'}
        />
      )}

      <Sheet onDismiss={() => setOpenSheet(null)} title="Workspace" visible={openSheet === 'workspace'}>
        <SheetRow
          description="Run in the project checkout"
          label="Work locally"
          onPress={pick(() => setIsolated(false))}
          selected={!isolated}
        />
        <SheetRow
          description="A separate branch and folder that never touches the checkout"
          disabled={projectless}
          label="Isolated worktree"
          onPress={pick(() => setIsolated(true))}
          selected={isolated}
        />
      </Sheet>

      <Sheet onDismiss={() => setOpenSheet(null)} title="Base branch" visible={openSheet === 'branch'}>
        {branches.isPending ? (
          <View style={styles.sheetLoading}>
            <ActivityIndicator color={theme.textTertiary} />
          </View>
        ) : branches.error ? (
          <Text style={[styles.sheetNote, { color: theme.danger }]}>
            {branches.error instanceof Error ? branches.error.message : String(branches.error)}
          </Text>
        ) : !branches.data ? (
          <Text style={[styles.sheetNote, { color: theme.textTertiary }]}>
            This project isn’t a Git repository.
          </Text>
        ) : (
          branches.data.branches.map((branch) => (
            <SheetRow
              description={branch.name === branches.data?.current ? 'Current branch' : undefined}
              key={branch.name}
              label={branch.name}
              onPress={pick(() => setBaseBranch(branch.name))}
              selected={branch.name === (baseBranch ?? branches.data?.default_branch ?? branches.data?.current)}
            />
          ))
        )}
      </Sheet>

      <RemoteProjectPicker
        visible={projectPickerOpen}
        onDismiss={() => setProjectPickerOpen(false)}
        onSelect={(project) => setProjectId(project.id)}
      />
      {localCommands.sheets}
      <ComposerContextPicker {...contextPicker.picker} />
    </KeyboardAvoidingView>
  );
}

const styles = StyleSheet.create({
  screen: { flex: 1 },
  home: { alignSelf: 'center', gap: 22, maxWidth: 760, paddingHorizontal: 16, width: '100%' },
  headline: {
    fontFamily: MonoFont,
    fontSize: 28,
    fontWeight: '500',
    lineHeight: 36,
    paddingHorizontal: 8,
    textAlign: 'center',
  },
  composer: { gap: 2 },
  banner: { marginBottom: 0, marginHorizontal: 0 },
  // Bleeds to the screen edges so a chip scrolled past the edge reads as more.
  contextScroller: { marginHorizontal: -16 },
  context: { paddingHorizontal: 22 },
  error: { borderRadius: Radius.medium, marginBottom: 8, padding: 11 },
  errorText: { fontSize: 13, fontWeight: '600', lineHeight: 18 },
  sheetLoading: { alignItems: 'center', paddingVertical: 14 },
  sheetNote: { fontSize: 13, lineHeight: 18, paddingHorizontal: 12, paddingVertical: 10 },
});
