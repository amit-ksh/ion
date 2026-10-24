// The pass: a pure timeline of one illustrative team run. Every limit below is
// Ion's real default (waku-protocol TeamRunLimits); the run itself is synthetic.
export const LIMITS = { tokens: 2_000_000, reworkRounds: 2, parallel: 4, childReview: true } as const

export const STATIONS = [-100, -50, 0, 50] as const
export const HARNESSES = ['Nebius Token Factory', 'Pi', 'OpenCode', 'Codex CLI'] as const
export const HOME_X = -158
export const MOUTH_X = -127
export const PASS_X = 100
export const ACCEPT_X = 145
export const DROP = 58
export const STROKE = 3.6
export const LOOP = 28
export const STILL = 11

export type Verdict = 'accept' | 'rework'

export interface Ticket {
  id: string
  station: number
  print: number
  run: [number, number]
  pass: number
  verdict: Verdict
  rate: number
  round: number
  reworkOf?: string
}

export const TICKETS: Ticket[] = [
  { id: '6E24', station: 3, print: 0.3, run: [2.6, 7.6], pass: 8.2, verdict: 'accept', rate: 38_000, round: 0 },
  { id: '80F1', station: 2, print: 1.3, run: [3.2, 9.6], pass: 10.6, verdict: 'accept', rate: 30_000, round: 0 },
  { id: '38E2', station: 1, print: 2.3, run: [3.8, 11.6], pass: 12.8, verdict: 'accept', rate: 26_000, round: 0 },
  { id: '8CE6', station: 0, print: 3.3, run: [4.4, 13.6], pass: 15.0, verdict: 'rework', rate: 24_000, round: 0 },
  { id: 'EA96', station: 0, print: 16.6, run: [17.7, 21.7], pass: 23.1, verdict: 'accept', rate: 34_000, round: 1, reworkOf: '8CE6' },
]

const REVIEW_TOKENS = 18_000
const EMERGE = 0.8
const STAMP = 0.8
const SLIDE = 0.5
const FALL = 0.6
const RESET: [number, number] = [26, 27.5]

const clamp = (value: number, lo = 0, hi = 1) => Math.min(hi, Math.max(lo, value))
const ease = (value: number) => {
  const x = clamp(value)
  return x * x * (3 - 2 * x)
}
const span = (t: number, a: number, b: number) => ease((t - a) / (b - a))
const lerp = (a: number, b: number, k: number) => a + (b - a) * k

export type Phase = 'waiting' | 'printing' | 'queued' | 'running' | 'in review' | 'accepted' | 'superseded'

export interface TicketState {
  x: number
  drop: number
  opacity: number
  phase: Phase
  lit: boolean
  ink: number
}

export function ticketAt(ticket: Ticket, t: number): TicketState {
  const { print, run, pass, verdict } = ticket
  const station = STATIONS[ticket.station]
  const contact = pass + STAMP / 2
  const ink = clamp((t - contact) / 0.12)
  let x = HOME_X
  let drop = 0
  let opacity = 1
  let phase: Phase = 'waiting'
  if (t < print) opacity = 0
  else if (t < print + EMERGE) {
    x = lerp(HOME_X, MOUTH_X, span(t, print, print + EMERGE))
    phase = 'printing'
  } else if (t < run[0]) {
    x = lerp(MOUTH_X, station, span(t, print + EMERGE, run[0]))
    phase = 'queued'
  } else if (t < run[1]) {
    x = station
    phase = 'running'
  } else if (t < pass + STAMP) {
    x = lerp(station, PASS_X, span(t, run[1], pass))
    phase = 'in review'
  } else if (verdict === 'accept') {
    x = lerp(PASS_X, ACCEPT_X, span(t, pass + STAMP, pass + STAMP + SLIDE))
    const fall = pass + STAMP + SLIDE
    drop = DROP * span(t, fall, fall + FALL)
    opacity = 1 - span(t, fall + FALL * 0.45, fall + FALL)
    phase = 'accepted'
  } else {
    x = PASS_X
    const fall = pass + STAMP
    drop = DROP * span(t, fall, fall + FALL)
    opacity = 1 - span(t, fall + FALL * 0.45, fall + FALL)
    phase = 'superseded'
  }
  return { x, drop, opacity, phase, lit: phase === 'in review', ink }
}

/** Stamp plunger travel toward the ticket, 0 at rest, STROKE at contact. */
export function stampAt(t: number) {
  for (const { pass } of TICKETS) {
    if (t >= pass && t <= pass + STAMP) return STROKE * Math.sin((Math.PI * (t - pass)) / STAMP) ** 2
  }
  return 0
}

function spent(t: number) {
  let tokens = 0
  for (const ticket of TICKETS) {
    tokens += ticket.rate * clamp(t - ticket.run[0], 0, ticket.run[1] - ticket.run[0])
    if (t >= ticket.pass + STAMP / 2) tokens += REVIEW_TOKENS
  }
  return tokens
}

/** Tokens the needle shows: the run total, then back to zero for the next run. */
export function tokensAt(t: number) {
  return spent(Math.min(t, RESET[0])) * (1 - span(t, RESET[0], RESET[1]))
}

export const RUN_TOTAL = spent(LOOP)

export function lampsAt(t: number) {
  const stations = STATIONS.map((_, index) => TICKETS.some((ticket) => ticket.station === index && t >= ticket.run[0] && t < ticket.run[1]))
  const printer = TICKETS.some((ticket) => t >= ticket.print && t < ticket.print + EMERGE)
  return { stations, printer }
}

/** Moments a reader can step between with the arrow keys. */
export const EVENTS = [
  0,
  ...TICKETS.flatMap((ticket) => [ticket.print + EMERGE, ticket.run[0], ticket.run[1], ticket.pass + STAMP / 2]),
  RESET[0],
].sort((a, b) => a - b)

const million = (tokens: number) => (tokens / 1_000_000).toFixed(2)
const budget = (t: number) => `${million(tokensAt(t))} / ${million(LIMITS.tokens)}M tok`

const LABEL: Record<Phase, string> = {
  waiting: 'waiting',
  printing: 'issued',
  queued: 'queued',
  running: 'running',
  'in review': 'in review',
  accepted: 'accepted',
  superseded: 'rework',
}

function latest(t: number) {
  let best: { ticket: Ticket; at: number; phase: Phase } | null = null
  for (const ticket of TICKETS) {
    const marks: Array<[number, Phase]> = [
      [ticket.print, 'printing'],
      [ticket.print + EMERGE, 'queued'],
      [ticket.run[0], 'running'],
      [ticket.run[1], 'in review'],
      [ticket.pass + STAMP / 2, ticket.verdict === 'accept' ? 'accepted' : 'superseded'],
    ]
    for (const [at, phase] of marks) if (at <= t && (!best || at >= best.at)) best = { ticket, at, phase }
  }
  return best
}

export function phraseAt(t: number) {
  if (t >= RESET[0]) return 'run complete · 4 accepted · 1 rework'
  const last = latest(t)
  if (!last) return 'new run · 4 stations'
  const { ticket, phase } = last
  if (phase === 'superseded') return `#${ticket.id} rework 1/${LIMITS.reworkRounds}`
  if (ticket.reworkOf && (phase === 'printing' || phase === 'queued')) return `#${ticket.id} replaces #${ticket.reworkOf}`
  return `#${ticket.id} ${LABEL[phase]}`
}

export const readoutAt = (t: number) => `${phraseAt(t)} · ${budget(t)}`
