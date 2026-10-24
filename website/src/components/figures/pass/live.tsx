import * as React from 'react'
import { IsoFigure } from '@/components/iso/draw'
import { follow, useInView, useLoop, useReadout } from '@/components/iso/live'
import { translateAlong } from '@/components/iso/iso-kit'
import { COUNTER, DUCT, FRAME, GAUGE, HOPPER_BACK, HOPPER_FRONT, HUB, PLUNGER, PRINTER, RAIL, STAMP, STATIONS, TICKETS, TRAY_BACK, TRAY_FRONT } from './art'
import { MOUTH_CLIP, needleAt } from './geometry'
import { EVENTS, HOME_X, LOOP, STILL, TICKETS as RUN, lampsAt, phraseAt, readoutAt, stampAt, ticketAt, tokensAt } from './model'
import { HEIGHT, P, WIDTH } from './view'

const LABEL =
  'An isometric drawing of a kitchen pass that stands for an Ion team run. A ticket printer issues each assignment onto an overhead rail. Four agent stations sit under the rail; a station lamp is lit while its agent runs. Finished tickets slide to the pass, where a stamp presses each one against a backing board for the parent to review. Accepted tickets drop into the done tray. A ticket sent back for rework drops into a hopper and a return duct to the printer, which issues a linked replacement for the same station with a punched rework hole. A dial gauge reads tokens spent against the run budget.'

const QUIET = 3600
const moveOf = (x: number, drop: number) => translateAlong(P, [x - HOME_X, 0, -drop])

export function PassLive() {
  const { anchor, say } = useReadout<HTMLDivElement>()
  const tickets = React.useRef<Array<SVGGElement | null>>([])
  const plunger = React.useRef<SVGGElement>(null)
  const needle = React.useRef<SVGPathElement>(null)
  const parts = React.useRef<{ lamps: Element[]; printer: Element | null; inks: Element[]; papers: Element[] } | null>(null)
  const state = React.useRef({ t: STILL, target: STILL, touring: false, quietUntil: 0 })
  const visible = useInView(anchor)

  const draw = React.useCallback(
    (t: number) => {
      const found = parts.current
      if (!found) return
      RUN.forEach((ticket, index) => {
        const now = ticketAt(ticket, t)
        const group = tickets.current[index]
        group?.setAttribute('transform', moveOf(now.x, now.drop))
        group?.setAttribute('opacity', now.opacity.toFixed(3))
        found.inks[index]?.setAttribute('opacity', now.ink.toFixed(2))
        found.papers[index]?.toggleAttribute('data-lit', now.lit)
      })
      plunger.current?.setAttribute('transform', translateAlong(P, [0, -stampAt(t), 0]))
      const lamps = lampsAt(t)
      found.lamps.forEach((lamp, index) => lamp.toggleAttribute('data-on', lamps.stations[index]!))
      found.printer?.toggleAttribute('data-on', lamps.printer)
      needle.current?.setAttribute('d', needleAt(tokensAt(t)))
      say(readoutAt(t))
    },
    [say],
  )

  const kick = useLoop((dt, calm) => {
    const s = state.current
    if (calm) {
      s.t = s.target
      draw(s.t)
      return false
    }
    if (s.touring) {
      s.target = (s.target + dt) % LOOP
      s.t = s.target
    } else {
      s.t = follow(s.t, s.target, dt, 0.28)
      if (performance.now() > s.quietUntil) s.touring = true
    }
    draw(s.t)
    return s.touring || s.t !== s.target || (s.quietUntil !== Infinity && performance.now() <= s.quietUntil)
  }, visible)

  React.useEffect(() => {
    const root = anchor.current
    if (!root) return
    parts.current = {
      lamps: STATIONS_INDEX.map((index) => root.querySelector(`[data-lamp="${index}"]`)!).filter(Boolean),
      printer: root.querySelector('[data-lamp="printer"]'),
      inks: tickets.current.map((group) => group?.querySelector('.pass-ink') ?? null).filter((node): node is SVGGElement => node !== null),
      papers: tickets.current.map((group) => group?.querySelector('.pass-paper > .iso-solid') ?? null).filter((node): node is Element => node !== null),
    }
    state.current.touring = !window.matchMedia('(prefers-reduced-motion: reduce)').matches
    draw(state.current.t)
    kick()
  }, [anchor, draw, kick])

  const steer = (target: number, announce = true, hold = QUIET) => {
    const s = state.current
    s.touring = false
    s.target = Math.min(LOOP - 0.001, Math.max(0, target))
    s.quietUntil = performance.now() + hold
    if (announce) anchor.current?.setAttribute('aria-valuetext', phraseAt(s.target))
    kick()
  }
  const step = (direction: 1 | -1) => {
    const now = state.current.touring ? state.current.t : state.current.target
    const next = direction > 0 ? EVENTS.find((at) => at > now + 0.05) ?? 0 : [...EVENTS].reverse().find((at) => at < now - 0.05) ?? EVENTS[EVENTS.length - 1]!
    steer(next)
  }

  return (
    <div
      ref={anchor}
      className="pass-stage"
      tabIndex={0}
      role="slider"
      aria-label="Team run timeline. Arrow keys step between events, Home restarts, Space pauses or resumes the tour."
      aria-valuemin={0}
      aria-valuemax={LOOP}
      aria-valuenow={STILL}
      aria-valuetext={phraseAt(STILL)}
      onPointerMove={(event) => {
        if (event.pointerType !== 'mouse') return
        const box = event.currentTarget.getBoundingClientRect()
        steer(((event.clientX - box.left) / box.width) * LOOP, false)
      }}
      onPointerDown={(event) => {
        if (event.pointerType !== 'mouse') step(1)
      }}
      onKeyDown={(event) => {
        if (event.key === 'ArrowRight' || event.key === 'ArrowUp') step(1)
        else if (event.key === 'ArrowLeft' || event.key === 'ArrowDown') step(-1)
        else if (event.key === 'Home') steer(0)
        else if (event.key === 'End') steer(EVENTS[EVENTS.length - 1]!)
        else if (event.key === ' ') {
          const s = state.current
          if (s.touring) steer(s.t, true, Infinity)
          else {
            s.touring = true
            s.target = s.t
            s.quietUntil = 0
            kick()
          }
        } else return
        event.preventDefault()
      }}
    >
      <IsoFigure width={WIDTH} height={HEIGHT} label={LABEL}>
        <defs>
          <clipPath id="pass-mouth">
            <path d={MOUTH_CLIP} />
          </clipPath>
          <radialGradient id="pass-steel" cx="0.36" cy="0.32" r="0.75">
            <stop offset="0" className="pass-steel-hi" />
            <stop offset="0.45" className="pass-steel-mid" />
            <stop offset="1" className="pass-steel-lo" />
          </radialGradient>
        </defs>
        {COUNTER}
        {PRINTER}
        {DUCT}
        {HOPPER_BACK}
        {TRAY_BACK}
        {FRAME}
        {RAIL}
        <g clipPath="url(#pass-mouth)">
          {TICKETS.map((art, index) => {
            const now = ticketAt(RUN[index]!, STILL)
            return (
              <g key={RUN[index]!.id} ref={(node) => void (tickets.current[index] = node)} transform={moveOf(now.x, now.drop)} opacity={now.opacity}>
                {art}
              </g>
            )
          })}
        </g>
        {HOPPER_FRONT}
        {TRAY_FRONT}
        {STATIONS}
        <g ref={plunger} transform={translateAlong(P, [0, -stampAt(STILL), 0])}>
          {PLUNGER}
        </g>
        {STAMP}
        {GAUGE}
        <path ref={needle} className="iso-line pass-needle" data-tone="lit" data-free="" d={needleAt(tokensAt(STILL))} />
        {HUB}
      </IsoFigure>
    </div>
  )
}

const STATIONS_INDEX = [0, 1, 2, 3]
