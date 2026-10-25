import type { AgentSession } from '@waku/client';
import { expect, test } from 'bun:test';

import { railSessions, ticketNumber, ticketState } from './ticket';

function session(id: string, status: AgentSession['status'], at: number, started = true) {
  return { id, status, created_at: at, last_reply_at: at, turns: [], messages: started ? [{}] : [], provider_cursor: null } as unknown as AgentSession;
}

test('rail leads with work that needs the user, then most recent', () => {
  const ready = session('ready', 'idle', 300);
  const waiting = session('waiting', 'waiting', 100);
  const working = session('working', 'working', 200);
  const older = session('older', 'working', 50);
  const unstarted = { ...session('unstarted', 'idle', 999, false), last_reply_at: null } as AgentSession;
  expect(railSessions([ready, older, working, waiting, unstarted]).map(item => item.id))
    .toEqual(['waiting', 'working', 'older', 'ready']);
  expect(railSessions([ready, working], 8, item => item.id === 'ready').map(item => item.id)).toEqual(['ready', 'working']);
  expect(railSessions([ready, older, working, waiting], 2)).toHaveLength(2);
});

test('ticket numbers are stable short ids and states carry desktop meaning', () => {
  expect(ticketNumber('abcdef01-0000-4000-8000-000000000000')).toBe('#ABCD');
  expect(ticketNumber('8ce6')).toBe('#8CE6');
  expect(ticketState('waiting')).toBe('needsYou');
  expect(ticketState('connecting')).toBe('working');
  expect(ticketState('idle')).toBe('ready');
});
