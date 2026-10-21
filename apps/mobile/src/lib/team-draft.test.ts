import { expect, test } from 'bun:test';
import { teamDraftProblem } from './team-draft';
const agents = new Set(['lead', 'writer', 'reviewer']);
test('accepts a hierarchy independently of member order', () => {
  expect(teamDraftProblem([{ agentId: 'reviewer', parentId: 'writer' }, { agentId: 'lead', parentId: null }, { agentId: 'writer', parentId: 'lead' }], agents)).toBeNull();
});
test('rejects cycles and dangling managers without hanging', () => {
  expect(teamDraftProblem([{ agentId: 'lead', parentId: null }, { agentId: 'writer', parentId: 'reviewer' }, { agentId: 'reviewer', parentId: 'writer' }], agents)).toContain('cycle');
  expect(teamDraftProblem([{ agentId: 'lead', parentId: null }, { agentId: 'writer', parentId: 'missing' }], agents)).toContain('manager');
});
test('requires one lead, unique members and current agents', () => {
  expect(teamDraftProblem([{ agentId: 'lead', parentId: null }, { agentId: 'writer', parentId: null }], agents)).toContain('one team lead');
  expect(teamDraftProblem([{ agentId: 'lead', parentId: null }, { agentId: 'lead', parentId: 'lead' }], agents)).toContain('once');
  expect(teamDraftProblem([{ agentId: 'missing', parentId: null }], agents)).toContain('unavailable');
});
