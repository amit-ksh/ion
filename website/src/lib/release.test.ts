import assert from 'node:assert/strict'
import { test } from 'bun:test'
import { downloadUrl, formatSize, releases } from './release'

test('unpublished, unsafe and inherited URLs stay unavailable', () => {
  for (const value of [null, '', 'not a URL', 'D:/builds/ion.exe', '/ion.apk', 'javascript:alert(1)', 'http://example.com/ion.exe', 'https://user:token@example.com/ion.apk', 'https://releases.waku.sh/Ion.exe', 'https://waku.sh/ion.apk', 'https://localhost/ion.exe']) {
    assert.equal(downloadUrl(value), null, String(value))
  }
})

test('explicit HTTPS artifact URLs retain their version and query', () => {
  assert.equal(downloadUrl('https://example.com/releases/ion-1.2.3.exe?download=1'), 'https://example.com/releases/ion-1.2.3.exe?download=1')
})

test('release records have independently versioned integrity metadata', () => {
  assert.deepEqual(releases.map((release) => release.platform), ['windows', 'android'])
  for (const release of releases) {
    assert.match(release.version, /^\d+\.\d+\.\d+$/)
    assert.match(release.sha256, /^[a-f0-9]{64}$/)
    assert.ok(Number.isSafeInteger(release.bytes) && release.bytes > 0)
    assert.ok(release.notes.length > 0)
  }
  assert.equal(formatSize(53387237), '53.4 MB')
})
