import metadata from './releases.json'

export interface Release {
  platform: 'windows' | 'android'
  name: string
  format: string
  version: string
  architecture: string
  bytes: number
  sha256: string
  url: string | null
  notes: string
}

// Publishing is explicit: never infer artifact names or borrow upstream releases.
export function downloadUrl(value: string | null): string | null {
  if (!value || value.trim() !== value) return null
  try {
    const url = new URL(value)
    if (url.protocol !== 'https:' || url.username || url.password) return null
    if (url.hostname === 'waku.sh' || url.hostname.endsWith('.waku.sh')) return null
    if (['localhost', '127.0.0.1', '[::1]'].includes(url.hostname)) return null
    return url.href
  } catch {
    return null
  }
}

export const releases: Release[] = (metadata as Release[]).map((release) => ({
  ...release,
  url: downloadUrl(release.url),
}))

export function formatSize(bytes: number): string {
  return `${(bytes / 1_000_000).toFixed(1)} MB`
}
