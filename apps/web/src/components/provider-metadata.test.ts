import { expect, test } from 'bun:test'
import { providerMeta } from './waku-icon'

test('native Antigravity history retains its own harness identity', () => {
  expect(providerMeta('antigravity')).toEqual({
    id: 'antigravity',
    name: 'Antigravity',
    shortName: 'Antigravity',
    command: 'agy',
  })
  expect(providerMeta('codex').id).toBe('codex')
  expect(providerMeta('nebius').id).toBe('nebius')
})
