import { useEffect, useRef, useState, type FormEvent } from 'react'
import { AnimatePresence, MotionConfig, motion } from 'framer-motion'
import { ArrowUp, Check, ChevronDown, Folder, GitBranch, Laptop, Lock, Mic, RotateCcw } from 'lucide-react'

type Stage = 'working' | 'needs' | 'ready'

const HARNESSES = [
  { name: 'Codex CLI', mark: 'openai' },
  { name: 'OpenCode', mark: 'opencode' },
  { name: 'Pi', mark: 'pi' },
  { name: 'Kimi Code', mark: 'kimi' },
  { name: 'Grok Build', mark: 'grok' },
  { name: 'Amp', mark: 'amp' },
] as const
type Harness = (typeof HARNESSES)[number]['name']

interface Item {
  id: string
  title: string
  harness: Harness
  stage: Stage
  started: number
  until: number
  rework: number
  auto: boolean
}

const MAX_REWORK = 2
const WORK_MS = 4200
const PASS_MS = 3400
const SCRIPT: Array<{ text: string; harness: Harness }> = [
  { text: 'Add keyboard navigation to the artifact grid', harness: 'Codex CLI' },
  { text: '@team:{Release crew} draft the 0.1.25 release notes', harness: 'OpenCode' },
  { text: 'Summarize open issues into a PDF report', harness: 'Pi' },
  { text: 'Fix the flaky retry test in the usage ledger', harness: 'Kimi Code' },
]

// Fixed ages so the server and first client render agree.
const SEED: Item[] = [
  { id: '8CE6', title: 'Add a theme toggle to the settings page', harness: 'Codex CLI', stage: 'working', started: -134_000, until: 2_600, rework: 0, auto: true },
  { id: '80F1', title: 'Draft onboarding copy for connected apps', harness: 'OpenCode', stage: 'needs', started: -362_000, until: 5_200, rework: 1, auto: true },
  { id: '6E24', title: 'Export the QA findings as a PDF', harness: 'Pi', stage: 'ready', started: -880_000, until: 0, rework: 0, auto: true },
  { id: '38E2', title: 'Rename the sidebar section to Work', harness: 'Grok Build', stage: 'ready', started: -1_265_000, until: 0, rework: 0, auto: true },
]

const markOf = (harness: Harness) => HARNESSES.find((entry) => entry.name === harness)!.mark
const clock = (ms: number) => {
  const seconds = Math.max(0, Math.floor(ms / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}
const hex = () => Math.floor(Math.random() * 0xffff).toString(16).toUpperCase().padStart(4, '0')
const wait = (ms: number) => new Promise((done) => setTimeout(done, ms))

function useInView<T extends Element>() {
  const ref = useRef<T>(null)
  const [inView, setInView] = useState(false)
  useEffect(() => {
    const node = ref.current
    if (!node) return
    const observer = new IntersectionObserver((entries) => setInView(Boolean(entries.at(-1)?.isIntersecting)), { rootMargin: '80px 0px' })
    observer.observe(node)
    return () => observer.disconnect()
  }, [])
  return { ref, inView }
}

export function HeroDemo() {
  const { ref, inView } = useInView<HTMLDivElement>()
  const [items, setItems] = useState(SEED)
  const [now, setNow] = useState(0)
  const [draft, setDraft] = useState('')
  const [harness, setHarness] = useState<Harness>('Codex CLI')
  const [pressed, setPressed] = useState(false)
  const [takenOver, setTakenOver] = useState(false)
  const [still, setStill] = useState(false)
  const epoch = useRef(0)

  useEffect(() => {
    const query = window.matchMedia('(prefers-reduced-motion: reduce)')
    const sync = () => setStill(query.matches)
    sync()
    query.addEventListener('change', sync)
    return () => query.removeEventListener('change', sync)
  }, [])

  // One clock drives every timer and stage change on the rail.
  useEffect(() => {
    if (!inView) return
    // Resume the demo clock where it stopped, so timers never jump after scrolling away.
    epoch.current = performance.now() - now
    const timer = setInterval(() => {
      const at = performance.now() - epoch.current
      setNow(at)
      setItems((list) => {
        let changed = false
        const next = list.map((item) => {
          if (item.stage === 'working' && at >= item.until) {
            changed = true
            return { ...item, stage: 'needs' as const, until: item.auto ? at + PASS_MS : Infinity }
          }
          if (item.stage === 'needs' && at >= item.until) {
            changed = true
            return { ...item, stage: 'ready' as const }
          }
          return item
        })
        return changed ? next : list
      })
    }, 500)
    return () => clearInterval(timer)
  }, [inView])

  const fire = (title: string, by: Harness, auto: boolean) => {
    const at = performance.now() - epoch.current
    setItems((list) => {
      const fresh: Item = { id: hex(), title, harness: by, stage: 'working', started: at, until: at + WORK_MS, rework: 0, auto }
      const kept = [fresh, ...list]
      if (kept.length <= 4) return kept
      const drop = kept.map((item) => item.stage).lastIndexOf('ready')
      return kept.filter((_, index) => index !== (drop > 0 ? drop : kept.length - 1))
    })
  }

  // The idle tour types a request, fires it, and lets the pass clear it.
  useEffect(() => {
    if (!inView || still || takenOver) return
    let live = true
    const run = async () => {
      for (let round = 0; live; round++) {
        const { text, harness: by } = SCRIPT[round % SCRIPT.length]!
        await wait(1600)
        if (!live) return
        setHarness(by)
        for (let index = 1; index <= text.length && live; index++) {
          setDraft(text.slice(0, index))
          await wait(26)
        }
        await wait(520)
        if (!live) return
        setPressed(true)
        await wait(160)
        setPressed(false)
        setDraft('')
        fire(text, by, true)
        await wait(6400)
      }
    }
    void run()
    return () => {
      live = false
      setPressed(false)
    }
  }, [inView, still, takenOver])

  const takeOver = () => {
    if (takenOver) return
    setTakenOver(true)
    setDraft('')
  }
  const submit = (event: FormEvent) => {
    event.preventDefault()
    const text = draft.trim()
    if (!text) return
    fire(text, harness, false)
    setDraft('')
  }
  const decide = (id: string, accept: boolean) => {
    takeOver()
    const at = performance.now() - epoch.current
    setItems((list) =>
      list.map((item) =>
        item.id !== id ? item : accept ? { ...item, stage: 'ready' } : { ...item, stage: 'working', rework: item.rework + 1, until: at + WORK_MS, auto: false },
      ),
    )
  }

  return (
    <MotionConfig reducedMotion="user">
      <div ref={ref} className="mx-auto w-full max-w-[1120px]">
        <p className="mono mx-auto max-w-[720px] text-center text-[clamp(1.375rem,3.4vw,2.5rem)] leading-[1.18] font-medium tracking-[-0.02em] text-ink">
          What should we build in <span className="underline decoration-[#9aa197] decoration-dotted decoration-2 underline-offset-[10px]">ion-site</span>?
        </p>

        <form onSubmit={submit} className="mx-auto mt-8 max-w-[720px] rounded-[22px] border border-transparent bg-white p-2 shadow-[var(--shadow-float)]">
          <div className="px-3 pt-3">
            <label htmlFor="hero-ask" className="sr-only">
              Request
            </label>
            <input
              id="hero-ask"
              value={draft}
              onChange={(event) => {
                takeOver()
                setDraft(event.target.value)
              }}
              onFocus={takeOver}
              placeholder="Do anything… type @ to add files, teams or apps"
              autoComplete="off"
              className="w-full bg-transparent text-[16px] text-ink outline-none placeholder:text-[#8b9188] focus-visible:outline-none"
            />
          </div>
          <div className="mt-3 flex items-center gap-2 px-1 pb-1">
            <label className="chip relative cursor-pointer">
              <span className={`provider-mark size-[15px] text-ink-2`} style={{ maskImage: `url(/providers/${markOf(harness)}.svg)`, WebkitMaskImage: `url(/providers/${markOf(harness)}.svg)` }} aria-hidden="true" />
              <span>{harness}</span>
              <ChevronDown aria-hidden="true" />
              <select
                aria-label="Harness"
                value={harness}
                onChange={(event) => {
                  takeOver()
                  setHarness(event.target.value as Harness)
                }}
                className="absolute inset-0 cursor-pointer opacity-0"
              >
                {HARNESSES.map((entry) => (
                  <option key={entry.name}>{entry.name}</option>
                ))}
              </select>
            </label>
            <span className="chip max-[480px]:hidden">
              <Lock aria-hidden="true" />
              Supervised
            </span>
            <span className="chip px-2.5 text-ink-2" title="Local dictation runs in the desktop app">
              <Mic aria-hidden="true" />
              <span className="sr-only">Dictation</span>
            </span>
            <button
              type="submit"
              aria-label="Send request"
              disabled={!draft.trim()}
              data-pressed={pressed || undefined}
              className="ml-auto inline-flex size-10 items-center justify-center rounded-full bg-lime text-lime-ink transition-[scale,background-color] duration-150 ease-out enabled:active:scale-[0.96] disabled:bg-[#eceee9] disabled:text-[#9aa197] data-[pressed]:scale-[0.96] data-[pressed]:bg-lime [@media(hover:hover)]:enabled:hover:bg-[var(--lime-hover)]"
            >
              <ArrowUp className="size-[18px]" strokeWidth={2.25} aria-hidden="true" />
            </button>
          </div>
        </form>

        <div className="mx-auto mt-3 flex max-w-[720px] flex-wrap items-center gap-x-5 gap-y-1 px-3 text-[13px] text-ink-2">
          <span className="inline-flex items-center gap-1.5">
            <Folder className="size-[15px]" strokeWidth={1.75} aria-hidden="true" />
            ion-site
          </span>
          <span className="inline-flex items-center gap-1.5">
            <Laptop className="size-[15px]" strokeWidth={1.75} aria-hidden="true" />
            Local
          </span>
          <span className="inline-flex items-center gap-1.5">
            <GitBranch className="size-[15px]" strokeWidth={1.75} aria-hidden="true" />
            main
          </span>
        </div>

        <div className="mt-14 flex items-baseline justify-between gap-4 px-1">
          <h2 className="text-[15px] font-semibold text-ink">Live work</h2>
          <p className="mono text-[11px] tracking-[0.04em] text-quiet uppercase">Illustration · synthetic tasks</p>
        </div>
        <ul className="mt-3 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <AnimatePresence initial={false} mode="popLayout">
            {items.map((item, index) => (
              <motion.li
                key={item.id}
                layout
                initial={{ opacity: 0, y: 12, filter: 'blur(4px)' }}
                animate={{ opacity: 1, y: 0, filter: 'blur(0px)' }}
                exit={{ opacity: 0, y: -12, filter: 'blur(4px)', transition: { duration: 0.15, ease: 'easeOut' } }}
                transition={{ duration: 0.3, ease: 'easeOut' }}
                className={index === 3 ? 'max-sm:hidden' : undefined}
              >
                <Ticket item={item} now={now} onDecide={decide} />
              </motion.li>
            ))}
          </AnimatePresence>
        </ul>
      </div>
    </MotionConfig>
  )
}

function Ticket({ item, now, onDecide }: { item: Item; now: number; onDecide: (id: string, accept: boolean) => void }) {
  const mark = markOf(item.harness)
  return (
    <article className="ticket flex h-full min-h-[152px] flex-col px-4 pt-3 pb-3.5" data-state={item.stage}>
      <div className="mono flex items-center justify-between text-[12px] text-quiet">
        <span>#{item.id}</span>
        <span className="tabular">{item.stage === 'ready' ? 'done' : clock(now - item.started)}</span>
      </div>
      <div className="ticket-rule mt-2.5" />
      <h3 className="mt-2.5 line-clamp-2 text-[15px] leading-[1.35] font-medium text-ink">{item.title}</h3>
      <div className="mt-auto flex items-center justify-between gap-2 pt-3">
        <span className="inline-flex min-w-0 items-center gap-1.5 text-[13px] text-ink-2">
          <span className="provider-mark size-[14px]" style={{ maskImage: `url(/providers/${mark}.svg)`, WebkitMaskImage: `url(/providers/${mark}.svg)` }} aria-hidden="true" />
          <span className="truncate">{item.harness}</span>
        </span>
        <Stamp stage={item.stage} rework={item.rework} />
      </div>
      {item.stage === 'needs' ? (
        <div className="mt-3 flex gap-2">
          <button
            type="button"
            onClick={() => onDecide(item.id, true)}
            className="inline-flex min-h-9 flex-1 items-center justify-center gap-1.5 rounded-lg bg-lime px-3 text-[13px] font-semibold text-lime-ink transition-[scale,background-color] duration-150 ease-out active:scale-[0.96] [@media(hover:hover)]:hover:bg-[var(--lime-hover)]"
          >
            <Check className="size-4" strokeWidth={2} aria-hidden="true" />
            Accept
          </button>
          <button
            type="button"
            onClick={() => onDecide(item.id, false)}
            disabled={item.rework >= MAX_REWORK}
            title={item.rework >= MAX_REWORK ? 'Rework limit reached' : undefined}
            className="inline-flex min-h-9 flex-1 items-center justify-center gap-1.5 rounded-lg bg-[#eef0eb] px-3 text-[13px] font-semibold text-ink transition-[scale,background-color] duration-150 ease-out enabled:active:scale-[0.96] disabled:text-[#9aa197] [@media(hover:hover)]:enabled:hover:bg-[#e3e6df]"
          >
            <RotateCcw className="size-4" strokeWidth={2} aria-hidden="true" />
            Send back
          </button>
        </div>
      ) : null}
    </article>
  )
}

function Stamp({ stage, rework }: { stage: Stage; rework: number }) {
  if (stage === 'working')
    return (
      <span className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-[#eef0eb] px-2.5 py-1 text-[12px] font-medium text-ink-2">
        <span className="working-dot size-1.5 rounded-full bg-ink-2" aria-hidden="true" />
        {rework ? `Rework ${rework}/${MAX_REWORK}` : 'Working'}
      </span>
    )
  if (stage === 'needs')
    return <span className="inline-flex shrink-0 items-center rounded-full bg-lime px-2.5 py-1 text-[12px] font-semibold text-lime-ink">Needs you</span>
  return (
    <span className="inline-flex shrink-0 items-center gap-1 rounded-full bg-[#e9f3dc] px-2.5 py-1 text-[12px] font-medium text-leaf">
      <Check className="size-3.5" strokeWidth={2.25} aria-hidden="true" />
      Ready
    </span>
  )
}
