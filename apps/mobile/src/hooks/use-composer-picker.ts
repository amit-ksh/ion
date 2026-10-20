import { useQuery, useQueryClient } from "@tanstack/react-query";
import type {
  FileEntry,
  ProviderKind,
  ReportedCommand,
  SlashCommand,
} from "@waku/client";
import {
  detectComposerTrigger,
  mergeComposerCommands,
  type ComposerAutocompleteRow,
  type ComposerTriggerKind,
} from "@waku/client/composer-autocomplete";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Keyboard, type TextInputProps } from "react-native";

import type { ComposerTextInputHandle } from "@/components/composer-text-input.types";
import { useDaemonSettings } from "@/hooks/use-daemon-data";
import {
  composerPickerSearchState,
  insertComposerItem,
  searchComposerRows,
  selectionAfterComposerEdit,
  type ComposerPickerResults,
  type ComposerSelection,
} from "@/lib/composer-completion";
import {
  daemonKeys,
  discoverComposerCommands,
  listComposerFiles,
} from "@/lib/daemon-api";
import { useDaemon } from "@/lib/daemon-context";

const NO_COMMANDS: SlashCommand[] = [];
const NO_REPORTED: ReportedCommand[] = [];
const NO_FILES: FileEntry[] = [];

/** The + menu and @ shortcuts share a searchable sheet. Defer presenting it
 * briefly so users can finish typing @@ or @@@ before the keyboard dismisses. */
export function useComposerPicker({
  text,
  onChangeText,
  provider,
  root,
  reported = NO_REPORTED,
  contextKey,
  invokeTeams = true,
}: {
  text: string;
  onChangeText: (text: string) => void;
  provider: ProviderKind | null;
  root: string | null;
  reported?: ReportedCommand[];
  contextKey: string;
  invokeTeams?: boolean;
}) {
  const daemon = useDaemon();
  const settings = useDaemonSettings();
  const queryClient = useQueryClient();
  const inputRef = useRef<ComposerTextInputHandle>(null);
  const setInputRef = useCallback((input: ComposerTextInputHandle | null) => {
    inputRef.current = input;
  }, []);
  const [kind, setKind] = useState<ComposerTriggerKind | null>(null);
  const [visible, setVisible] = useState(false);
  const [query, setQuery] = useState("");
  const [caret, setCaret] = useState({
    text,
    selection: { start: text.length, end: text.length },
  });
  const [forcedSelection, setForcedSelection] = useState<ComposerSelection>();
  const insertionPoint = useRef<{
    text: string;
    selection: ComposerSelection;
  } | null>(null);
  const focusAfterDismiss = useRef(false);
  const openTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const cancelPendingOpen = useCallback(() => {
    if (openTimer.current !== null) clearTimeout(openTimer.current);
    openTimer.current = null;
  }, []);
  const profileId = daemon.activeProfile?.id ?? "disconnected";
  const binaryOverride = provider
    ? (settings.data?.provider_binary_overrides?.[provider] ?? null)
    : null;
  const enabled =
    daemon.phase === "connected" && Boolean(daemon.client && provider && root);
  const commandOptions = {
    queryKey: daemonKeys.composerCommands(
      profileId,
      provider,
      root,
      binaryOverride,
    ),
    queryFn: () =>
      discoverComposerCommands(
        daemon.client!,
        provider!,
        root!,
        binaryOverride,
      ),
    staleTime: 60_000,
  };
  const catalog = useQuery({
    ...commandOptions,
    enabled: enabled && visible && Boolean(settings.data) && kind === "command",
  });
  const discovered = catalog.data ?? NO_COMMANDS;
  const commands = useMemo(
    () => mergeComposerCommands(discovered, reported),
    [discovered, reported],
  );
  const files = useQuery({
    queryKey: daemonKeys.composerFiles(profileId, root),
    queryFn: () => listComposerFiles(daemon.client!, root!),
    enabled: enabled && visible && kind === "file",
    staleTime: 30_000,
  });
  const references = useQuery({
    queryKey: ["daemon", profileId, "composer-references"],
    enabled: enabled && visible && kind !== "command",
    staleTime: 30_000,
    queryFn: async () => {
      const [controlResult, appsResult] = await Promise.allSettled([
        daemon.client!.request({ type: "loadWorkspaceControl" }),
        daemon.client!.request({
          type: "workspace",
          operation: { type: "listComposerApps" },
        }),
      ]);
      if (
        controlResult.status !== "fulfilled" ||
        controlResult.value.type !== "workspaceControl"
      )
        throw new Error("Could not load teams. Reconnect and try again.");
      const apps =
        appsResult.status === "fulfilled" &&
        appsResult.value.type === "workspace" &&
        appsResult.value.result.type === "composerApps"
          ? appsResult.value.result.apps
          : null;
      return {
        teams: controlResult.value.snapshot.records.filter(
          (item) => item.kind === "team",
        ),
        apps: apps ?? [],
        warning:
          apps === null
            ? "Apps unavailable. Check connections in Apps, then retry."
            : null,
      };
    },
  });
  const fileIndex = files.data ?? NO_FILES;
  const scope = JSON.stringify([
    profileId,
    contextKey,
    root,
    provider,
    binaryOverride,
    kind,
  ]);
  const [results, setResults] = useState<ComposerPickerResults | null>(null);
  useEffect(() => {
    if (!kind || !visible) return;
    let cancelled = false;
    const search = searchComposerRows(
      { kind, query, start: 0, end: 0 },
      commands,
      fileIndex,
      () => cancelled,
      undefined,
      {
        apps: references.data?.apps ?? [],
        teams: references.data?.teams ?? [],
        allowTeams: true,
        invokeTeams,
      },
    );
    void search.then((rows) => {
      if (rows && !cancelled)
        setResults({ scope, query, commands, files: fileIndex, rows });
    });
    return () => {
      cancelled = true;
    };
  }, [
    commands,
    fileIndex,
    scope,
    kind,
    query,
    visible,
    references.data,
    invokeTeams,
  ]);

  useEffect(() => {
    cancelPendingOpen();
    setVisible(false);
    setQuery("");
    setForcedSelection(undefined);
    focusAfterDismiss.current = false;
    insertionPoint.current = null;
    return cancelPendingOpen;
  }, [contextKey, profileId, provider, root, enabled, cancelPendingOpen]);

  const search = composerPickerSearchState(
    results,
    scope,
    query,
    commands,
    fileIndex,
  );
  const source =
    kind === "file" ? files : kind === "command" ? catalog : references;
  const error = !enabled
    ? new Error("Connect to a daemon and choose a project first")
    : (source.error ?? (kind === "command" ? settings.error : null));
  const selection =
    caret.text === text
      ? caret.selection
      : { start: text.length, end: text.length };

  return {
    open(next: ComposerTriggerKind) {
      if (!enabled) return;
      cancelPendingOpen();
      // Native menu / keyboard dismissal can move the field's selection.
      // Keep the insertion point the user had when opening the picker.
      insertionPoint.current = { text, selection };
      Keyboard.dismiss();
      setQuery("");
      setKind(next);
      setVisible(true);
    },
    async getCommands() {
      if (!enabled)
        throw new Error("Connect to a daemon and choose a project first");
      // Sending a chosen or typed command also waits for its catalog, so
      // project templates cannot be bypassed by a quick send.
      const readySettings = settings.data ?? (await settings.refetch()).data;
      if (!readySettings)
        throw settings.error ?? new Error("Could not load daemon settings");
      const override =
        readySettings.provider_binary_overrides?.[provider!] ?? null;
      const discoveredCommands = await queryClient.fetchQuery({
        ...commandOptions,
        queryKey: daemonKeys.composerCommands(
          profileId,
          provider,
          root,
          override,
        ),
        queryFn: () =>
          discoverComposerCommands(daemon.client!, provider!, root!, override),
      });
      return mergeComposerCommands(discoveredCommands, reported);
    },
    inputProps: {
      inputRef: setInputRef,
      onChangeText: (value) => {
        setForcedSelection(undefined);
        setCaret({
          text: value,
          selection: selectionAfterComposerEdit(text, value, selection),
        });
        onChangeText(value);
        cancelPendingOpen();
        const nextCaret = selectionAfterComposerEdit(text, value, selection);
        const trigger = detectComposerTrigger(value, nextCaret.start);
        if (trigger && trigger.kind !== "command" && enabled) {
          openTimer.current = setTimeout(() => {
            openTimer.current = null;
            insertionPoint.current = { text: value, selection: nextCaret };
            setKind(trigger.kind);
            setQuery(trigger.query);
            setVisible(true);
            Keyboard.dismiss();
          }, 300);
        }
      },
      onSelectionChange: (event) => {
        const next = event.nativeEvent.selection;
        if (
          forcedSelection &&
          (next.start !== forcedSelection.start ||
            next.end !== forcedSelection.end)
        )
          return;
        setForcedSelection(undefined);
        setCaret({ text, selection: next });
      },
      selection: forcedSelection,
    } satisfies TextInputProps & { inputRef: typeof setInputRef },
    picker: {
      kind,
      visible,
      query,
      onQueryChange: setQuery,
      rows: search.rows,
      hasResults: search.hasResults,
      loading:
        !error &&
        (source.isFetching ||
          (kind !== "command" && references.isFetching) ||
          (kind === "command" && settings.isPending) ||
          search.pending),
      warning:
        kind === "file" || kind === "app" ? references.data?.warning : null,
      error: error
        ? error instanceof Error
          ? error.message
          : String(error)
        : null,
      onSelect: (row: ComposerAutocompleteRow) => {
        cancelPendingOpen();
        const target =
          insertionPoint.current?.text === text
            ? insertionPoint.current.selection
            : selection;
        const trigger = detectComposerTrigger(text, target.start);
        const selected =
          row.kind === "team" &&
          row.invoke &&
          text.slice(0, trigger?.start ?? target.start).trim()
            ? { ...row, invoke: false }
            : row;
        const replacement = insertComposerItem(text, target, selected);
        const nextSelection = {
          start: replacement.cursor,
          end: replacement.cursor,
        };
        setCaret({ text: replacement.text, selection: nextSelection });
        setForcedSelection(nextSelection);
        onChangeText(replacement.text);
        focusAfterDismiss.current = true;
        setVisible(false);
      },
      onDismiss: () => {
        cancelPendingOpen();
        setVisible(false);
        if (focusAfterDismiss.current) {
          focusAfterDismiss.current = false;
          inputRef.current?.focus();
        }
      },
      onRetry: () => {
        if (!enabled) return;
        if (kind === "command" && !settings.data) void settings.refetch();
        else {
          void source.refetch();
          if (kind === "file") void references.refetch();
        }
      },
    },
  };
}
