import { describe, expect, test } from 'bun:test'
import { fuzzyScore, shouldKeepPreviousPaletteItems } from './palette-search'

describe('command palette search parity', () => {
  test('ranks contiguous and boundary matches above loose subsequences', () => {
    expect(fuzzyScore('ion', 'ion daemon')!).toBeGreaterThan(
      fuzzyScore('ion', 'inspect operation network')!,
    )
    expect(fuzzyScore('id', 'ion daemon')).not.toBeNull()
    expect(fuzzyScore('missing', 'ion daemon')).toBeNull()
  })

  test('keeps useful rows during a pending transcript search', () => {
    expect(shouldKeepPreviousPaletteItems(0, true, 4)).toBe(true)
    expect(shouldKeepPreviousPaletteItems(1, true, 4)).toBe(false)
    expect(shouldKeepPreviousPaletteItems(0, false, 4)).toBe(false)
  })
})
