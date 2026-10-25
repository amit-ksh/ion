import type { AgentSession, SessionStatus } from '@waku/client';

import { sessionHasStarted, sessionTimestamp } from './session-presentation';

/**
 * What a ticket on the rail is waiting for, mirrored from desktop
 * `src/app/home.rs`. The array order is the rail order: work that needs the
 * user leads, settled work trails.
 */
export const TICKET_STATES = ['needsYou', 'working', 'background', 'failed', 'stopped', 'ready'] as const;
export type TicketState = typeof TICKET_STATES[number];

export const TICKET_LABELS: Record<TicketState, string> = {
  needsYou: 'Needs you',
  working: 'Working',
  background: 'Background',
  failed: 'Failed',
  stopped: 'Stopped',
  ready: 'Ready',
};

export function ticketState(status: SessionStatus): TicketState {
  switch (status) {
    case 'waiting': return 'needsYou';
    case 'connecting':
    case 'working': return 'working';
    case 'background': return 'background';
    case 'failed': return 'failed';
    default: return 'ready';
  }
}

/** A session's ticket state. A turn the user stopped is neither a failure
 * nor a result; list skeletons carry no turns and fall back to the status. */
export function sessionTicketState(session: Pick<AgentSession, 'status' | 'turns'>): TicketState {
  const state = ticketState(session.status);
  return (state === 'failed' || state === 'ready') && session.turns.at(-1)?.status === 'interrupted'
    ? 'stopped'
    : state;
}

/** A ticket's short order number: the first four hex digits of its id. */
export function ticketNumber(id: string): string {
  return `#${id.replace(/[^0-9a-f]/gi, '').slice(0, 4).toUpperCase()}`;
}

/** Started sessions in rail order: state first, then most recent activity. */
export function railSessions(
  sessions: readonly AgentSession[],
  limit = Number.POSITIVE_INFINITY,
  /** Pending approvals/questions the runtime holds count as Needs you too. */
  needsYou: (session: AgentSession) => boolean = () => false,
): AgentSession[] {
  const rank = (session: AgentSession) => needsYou(session) ? 0 : TICKET_STATES.indexOf(sessionTicketState(session));
  return sessions
    .filter(sessionHasStarted)
    .sort((a, b) => rank(a) - rank(b) || sessionTimestamp(b) - sessionTimestamp(a))
    .slice(0, limit);
}
