import { useEffect, useRef, useState } from 'react';
import type { ControlRecord, ControlSnapshot } from '@waku/client';
import { useAutomationQuery } from './use-automation-query';
import { definitionWriteRevision } from '@/lib/definition-write';

/** Revision-checked writes stay bound to the selected owner connection. */
export function useWorkspaceDefinition(id?: string) {
  const state = useAutomationQuery(['definition', id ?? 'new'], async client => {
    const response = await client.request({ type: 'loadWorkspaceControl' });
    if (response.type !== 'workspaceControl') throw new Error('Could not load workspace definitions.');
    return response.snapshot;
  }, { poll: false });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [draftRevision, bindDraftRevision] = useState<number | null>(null);
  const lock = useRef(false);
  const operation = useRef(0);
  const scope = `${state.daemon.activeProfile?.id}:${state.daemon.activeProfile?.address}`;
  const active = useRef({ scope, client: state.daemon.client });
  active.current = { scope, client: state.daemon.client };
  useEffect(() => () => { operation.current++; active.current = { scope: '', client: null }; }, []);
  useEffect(() => { operation.current++; lock.current = false; setBusy(false); setError(null); }, [scope, state.daemon.client]);
  const conflict = Boolean(id && draftRevision !== null && state.data && draftRevision !== state.data.revision);
  async function reloadDraft(apply: (snapshot: ControlSnapshot) => void) {
    const started = active.current;
    const result = await state.query.refetch();
    if (active.current.scope !== started.scope || active.current.client !== started.client || result.isError || !result.data || result.data.client !== started.client) return;
    bindDraftRevision(result.data.value.revision);
    apply(result.data.value);
    setError(null);
  }
  async function save(record: ControlRecord) {
    const client = state.daemon.client;
    if (!state.fresh || !state.data || !client || lock.current) return false;
    lock.current = true; setBusy(true); setError(null);
    const ticket = ++operation.current;
    try {
      const expectedRevision = definitionWriteRevision({ editing: Boolean(id), draftRevision, currentRevision: state.data.revision,
        recordExists: state.data.records.some(existing => existing.id === record.id) });
      const response = await client.request({ type: 'saveWorkspaceRecord', expectedRevision, record });
      if (response.type !== 'workspaceControl') throw new Error('The definition was not confirmed. Refresh before retrying.');
      if (ticket !== operation.current || active.current.scope !== scope || active.current.client !== client) return false;
      await state.query.refetch();
      return ticket === operation.current && active.current.scope === scope && active.current.client === client;
    } catch (cause) {
      if (ticket === operation.current && active.current.scope === scope && active.current.client === client)
        setError(cause instanceof Error ? cause.message : 'Could not save. Refresh and retry.');
      return false;
    } finally {
      if (ticket === operation.current && active.current.scope === scope && active.current.client === client) { lock.current = false; setBusy(false); }
    }
  }
  return { ...state, save, busy, error, scope, conflict, bindDraftRevision, reloadDraft };
}
