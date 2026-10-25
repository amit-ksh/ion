/**
 * Human time labels for daemon timestamps (unix seconds). Screens never show a
 * raw timestamp: past moments read as "5m ago", upcoming ones as "in 2h", and
 * absolute moments use the phone's local clock ("Today 17:05").
 */
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

function span(seconds: number): string {
  if (seconds < 3_600) return `${Math.max(1, Math.floor(seconds / 60))}m`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3_600)}h`;
  return `${Math.floor(seconds / 86_400)}d`;
}

/** "just now", "5m ago", "in 3h". */
export function relativeTimeLabel(seconds: number, now = Date.now()): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return 'Unknown time';
  const delta = seconds - Math.floor(now / 1_000);
  if (Math.abs(delta) < 60) return 'just now';
  return delta > 0 ? `in ${span(delta)}` : `${span(-delta)} ago`;
}

function clock(date: Date): string {
  return `${String(date.getHours()).padStart(2, '0')}:${String(date.getMinutes()).padStart(2, '0')}`;
}

function dayIndex(date: Date): number {
  return Math.floor(new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime() / 86_400_000);
}

/** Local wall-clock label: "Today 17:05", "Tomorrow 09:00", "Oct 26, 09:00", "Oct 26, 2027". */
export function localTimeLabel(seconds: number, now = Date.now()): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return 'Unknown time';
  const date = new Date(seconds * 1_000);
  const today = new Date(now);
  const days = dayIndex(date) - dayIndex(today);
  if (days === 0) return `Today ${clock(date)}`;
  if (days === 1) return `Tomorrow ${clock(date)}`;
  if (days === -1) return `Yesterday ${clock(date)}`;
  const day = `${MONTHS[date.getMonth()]} ${date.getDate()}`;
  return date.getFullYear() === today.getFullYear() ? `${day}, ${clock(date)}` : `${day}, ${date.getFullYear()}`;
}
