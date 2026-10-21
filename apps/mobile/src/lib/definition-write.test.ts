import { expect, test } from 'bun:test';
import { definitionWriteRevision } from './definition-write';
test('existing drafts cannot adopt the revision of an unseen desktop edit', () => {
  expect(definitionWriteRevision({ editing: true, draftRevision: 4, currentRevision: 4, recordExists: true })).toBe(4);
  expect(() => definitionWriteRevision({ editing: true, draftRevision: 4, currentRevision: 5, recordExists: true })).toThrow('workspace changed');
  expect(() => definitionWriteRevision({ editing: true, draftRevision: null, currentRevision: 5, recordExists: true })).toThrow();
});
test('new definitions can include newly added agents without overwriting an uncertain earlier save', () => {
  expect(definitionWriteRevision({ editing: false, draftRevision: 4, currentRevision: 5, recordExists: false })).toBe(5);
  expect(() => definitionWriteRevision({ editing: false, draftRevision: 4, currentRevision: 5, recordExists: true })).toThrow('already exists');
});
