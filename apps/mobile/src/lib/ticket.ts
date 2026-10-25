import type { AgentSession, SessionStatus } from '@waku/client';

import { sessionHasStarted, sessionTimestamp } from './session-presentation';

/**
 * What a ticket on the rail is waiting for, mirrored from desktop
 * `src/app/home.rs`. The array order is the rail order: work that needs the
 * user leads, settled work trails.
 */
export const TICKET_STATES = ['needsYou', 'working', 'background', 'failed', 'ready'] as const;
export type TicketState = typeof TICKET_STATES[number];

export const TICKET_LABELS: Record<TicketState, string> = {
  needsYou: 'Needs you',
  working: 'Working',
  background: 'Background',
  failed: 'Failed',
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
  const rank = (session: AgentSession) => needsYou(session) ? 0 : TICKET_STATES.indexOf(ticketState(session.status));
  return sessions
    .filter(sessionHasStarted)
    .sort((a, b) => rank(a) - rank(b) || sessionTimestamp(b) - sessionTimestamp(a))
    .slice(0, limit);
}
