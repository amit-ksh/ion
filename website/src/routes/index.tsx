import { useState, type ReactNode } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { ArrowRight, ArrowUpRight, Check, Copy, Download, KeyRound, Lock, Monitor, ShieldCheck, Smartphone } from 'lucide-react'
import { releases, formatSize } from '@/lib/release'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { HeroDemo } from '@/components/site/hero-demo'
import { PassFigure } from '@/components/figures/pass/figure'

export const Route = createFileRoute('/')({ component: Home })

const NAV = [
  ['How it works', '#how'],
  ['Teams', '#teams'],
  ['Workflows', '#workflows'],
  ['Artifacts', '#artifacts'],
  ['Download', '#downloads'],
  ['FAQ', '#faq'],
] as const

const AGENTS: Array<[string, string | null]> = [
  ['Amp', 'amp'],
  ['Claude Code', 'claude'],
  ['Codex CLI', 'openai'],
  ['Cursor CLI', 'cursor'],
  ['Fx', null],
  ['Grok Build', 'grok'],
  ['Kimi Code', 'kimi'],
  ['OpenCode', 'opencode'],
  ['Pi', 'pi'],
]

function B({ children }: { children: ReactNode }) {
  return <span className="bracket">[{children}]</span>
}

function BlockLink({ href, tone = 'lime', children }: { href: string; tone?: 'lime' | 'grey' | 'ink' | 'night'; children: ReactNode }) {
  return (
    <a href={href} className="block-link" data-tone={tone}>
      <span>{children}</span>
      <ArrowUpRight className="size-[18px]" strokeWidth={2} aria-hidden="true" />
    </a>
  )
}

function Mark({ name, className = 'size-4' }: { name: string; className?: string }) {
  return <span className={`provider-mark ${className}`} style={{ maskImage: `url(/providers/${name}.svg)`, WebkitMaskImage: `url(/providers/${name}.svg)` }} aria-hidden="true" />
}

function SectionHead({ id, title, children }: { id: string; title: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-1 gap-x-10 gap-y-6 lg:grid-cols-12 lg:items-end">
      <h2 id={id} className="display display-lg lg:col-span-8">
        {title}
      </h2>
      <div className="max-w-[46ch] text-[17px] leading-[1.6] text-ink-2 lg:col-span-4 lg:justify-self-end">{children}</div>
    </div>
  )
}

const container = 'mx-auto w-full max-w-[1440px] px-4 sm:px-6 lg:px-10'

function Home() {
  const publishedCount = releases.filter((release) => release.url).length
  return (
    <div className="ion-site min-h-dvh bg-white text-ink">
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <Nav />
      <main id="main">
        <Hero />
        <Agents />
        <HowItWorks />
        <Teams />
        <Workflows />
        <Artifacts />
        <Apps />
        <SpeechAndPhone />
        <Nebius />
        <Downloads publishedCount={publishedCount} />
        <Faq publishedCount={publishedCount} />
      </main>
      <Footer />
    </div>
  )
}

function Nav() {
  return (
    <header className="sticky top-0 z-40 border-b border-rule bg-white">
      <div className={`${container} flex h-16 items-center justify-between gap-6`}>
        <a href="/" aria-label="Ion home" className="inline-flex items-center gap-2.5">
          <span className="flex size-8 items-center justify-center rounded-[9px] bg-lime text-lime-ink">
            <span className="ion-mark size-5" aria-hidden="true" />
          </span>
          <span className="text-[20px] font-semibold tracking-[-0.02em]">ion</span>
        </a>
        <nav aria-label="Main" className="hidden items-center gap-1 lg:flex">
          {NAV.map(([label, href]) => (
            <a key={href} href={href} className="text-link inline-flex min-h-11 items-center px-3 text-[13px] font-medium tracking-[0.06em] text-ink-2 uppercase">
              {label}
            </a>
          ))}
        </nav>
        <BlockLink href="#downloads">Get Ion</BlockLink>
      </div>
    </header>
  )
}

function Hero() {
  return (
    <section aria-labelledby="hero-title" className="pt-14 sm:pt-20">
      <div className={container}>
        <div className="grid grid-cols-1 gap-x-10 gap-y-10 lg:grid-cols-12 lg:items-end">
          <h1 id="hero-title" className="display display-xl lg:col-span-8">
            Turn requests into <B>reviewed</B> work
          </h1>
          <div className="lg:col-span-4 lg:pb-2">
            <p className="max-w-[40ch] text-[18px] leading-[1.55] text-ink-2">
              Ion is a native desktop IDE, with an Android companion, for coordinating coding agents. Every request becomes a ticket on a live rail. Agents and teams work it; you decide what passes.
            </p>
            <div className="mt-7 flex flex-wrap gap-3">
              <BlockLink href="#downloads">Get Ion for Windows</BlockLink>
              <BlockLink href="#how" tone="grey">
                See the pass
              </BlockLink>
            </div>
          </div>
        </div>
      </div>
      <div className="glow-field mt-14 border-y border-rule px-4 pt-14 pb-12 sm:mt-20 sm:px-6 sm:pt-20 sm:pb-16">
        <HeroDemo />
      </div>
    </section>
  )
}

function Agents() {
  return (
    <section aria-labelledby="agents-title" className="border-b border-rule">
      <div className={`${container} grid grid-cols-1 gap-8 py-12 lg:grid-cols-12 lg:items-center`}>
        <h2 id="agents-title" className="text-[15px] leading-[1.5] font-semibold text-ink lg:col-span-3">
          Works with the agent CLIs you already use.
          <span className="mt-1 block font-normal text-quiet">Ion detects installed, authenticated CLIs and speaks each one's own protocol.</span>
        </h2>
        <ul className="grid grid-cols-2 border-t border-l border-rule sm:grid-cols-3 lg:col-span-9 lg:grid-cols-9">
          {AGENTS.map(([name, mark]) => (
            <li key={name} className="flex min-h-[84px] flex-col justify-between gap-3 border-r border-b border-rule p-3">
              {mark ? <Mark name={mark} className="size-5 text-ink" /> : <span className="mono text-[15px] leading-5 font-semibold text-ink">fx</span>}
              <span className="text-[13px] font-medium text-ink-2">{name}</span>
            </li>
          ))}
        </ul>
      </div>
    </section>
  )
}

const STEPS = [
  ['Issue', 'A request, or a lead agent delegating part of one, becomes an assignment with its own task, harness and limits.'],
  ['Run', 'Each agent works at its own station: a local CLI or a Nebius sandbox. A team runs up to four assignments at once by default.'],
  ['Review', 'Nothing is done because a model says so. The parent reads the submitted result and its evidence, then accepts it or sends it back.'],
  ['Rework', 'A rejected result is superseded by a linked replacement for the same agent, inside a bounded number of rounds and a token budget.'],
]

function HowItWorks() {
  return (
    <section id="how" aria-labelledby="how-title" className="border-b border-rule bg-canvas py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="how-title" title={<>Every request is a <B>ticket</B></>}>
          Ion runs agent work like a kitchen pass. Watch one team run: tickets clip onto the rail, light a station while an agent works, and stop at the pass for review.
        </SectionHead>
        <div className="mt-14 grid grid-cols-1 gap-12 lg:grid-cols-12">
          <ol className="lg:col-span-4">
            {STEPS.map(([title, text], index) => (
              <li key={title} className="grid grid-cols-[48px_1fr] gap-x-3 border-t border-ink py-5 last:border-b">
                <span className="mono pt-0.5 text-[13px] text-quiet">0{index + 1}</span>
                <div>
                  <h3 className="text-[17px] font-semibold tracking-[-0.01em] text-ink">{title}</h3>
                  <p className="mt-1.5 text-[15px] leading-[1.6] text-ink-2">{text}</p>
                </div>
              </li>
            ))}
          </ol>
          <div className="min-w-0 lg:col-span-8">
            <PassFigure />
          </div>
        </div>
      </div>
    </section>
  )
}

const ORG = {
  lead: { role: 'Lead', id: 'A1', harness: 'Codex CLI', mark: 'openai', env: 'Local' },
  reports: [
    { role: 'Researcher', id: 'B2', harness: 'Pi', mark: 'pi', env: 'Local', state: 'Running' },
    { role: 'Engineer', id: 'C3', harness: 'OpenCode', mark: 'opencode', env: 'Nebius sandbox', state: 'In review' },
    { role: 'Reviewer', id: 'D4', harness: 'Kimi Code', mark: 'kimi', env: 'Local', state: 'Queued' },
  ],
}

const LIMIT_ROWS: Array<[string, string, string]> = [
  ['Work items', '64', '1-256'],
  ['Parallel tasks', '4', '1-16'],
  ['Delegation depth', '8', '1-16'],
  ['Attempts', '2', '1-8'],
  ['Rework rounds', '2', '0-8'],
  ['Model calls', '64', '1-4,096'],
  ['Token budget', '2,000,000', 'up to 1B'],
  ['Timeout', '60 min', '1 min-7 days'],
]

function Teams() {
  return (
    <section id="teams" aria-labelledby="teams-title" className="border-b border-rule py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="teams-title" title={<>Agent teams with an <B>org chart</B></>}>
          Save agents with their own instructions, harness, model and environment, then arrange them under a lead. Address a team from the composer with <code className="mono rounded bg-canvas px-1.5 py-0.5 text-[14px] text-ink">@team:{'{name}'}</code>.
        </SectionHead>

        <div className="mt-14 grid grid-cols-1 gap-10 lg:grid-cols-12">
          <figure className="lg:col-span-7">
            <div className="rounded-[20px] bg-canvas p-5 sm:p-8">
              <div className="mx-auto max-w-[300px]">
                <OrgCard {...ORG.lead} state="Delegating" lead />
              </div>
              <div className="org-rail relative mt-3 grid grid-cols-1 gap-3 md:mt-9 md:grid-cols-3">
                {ORG.reports.map((agent) => (
                  <div key={agent.id} className="org-branch">
                    <OrgCard {...agent} />
                  </div>
                ))}
              </div>
              <ReviewTrail />
            </div>
            <figcaption className="mt-3 text-[13px] text-quiet">
              Illustration of a saved team. Each member keeps its own harness and environment; there is no team-wide forced harness.
            </figcaption>
          </figure>

          <div className="lg:col-span-5">
            <h3 className="text-[17px] font-semibold text-ink">Limits for every run</h3>
            <p className="mt-1.5 text-[15px] leading-[1.6] text-ink-2">A run pins its team, profiles and limits when it starts. These are the defaults, and the range each one accepts.</p>
            <table className="mt-5 w-full border-collapse text-left">
              <thead>
                <tr className="border-b border-ink text-[12px] tracking-[0.06em] text-quiet uppercase">
                  <th scope="col" className="py-2 font-medium">Limit</th>
                  <th scope="col" className="py-2 text-right font-medium">Default</th>
                  <th scope="col" className="py-2 text-right font-medium">Range</th>
                </tr>
              </thead>
              <tbody>
                {LIMIT_ROWS.map(([name, value, range]) => (
                  <tr key={name} className="border-b border-rule">
                    <th scope="row" className="py-2.5 text-[15px] font-medium text-ink">
                      {name}
                    </th>
                    <td className="mono py-2.5 text-right text-[14px] text-ink">{value}</td>
                    <td className="mono py-2.5 text-right text-[13px] text-quiet">{range}</td>
                  </tr>
                ))}
              </tbody>
            </table>
            <ul className="mt-6 space-y-2.5 text-[15px] leading-[1.55] text-ink-2">
              {[
                'Grant each agent workspace read or write, shell, network and exact connected-app actions.',
                'Child review is on by default: a parent accepts a result or sends it back with findings.',
                'Stop halts new dispatch and cancels every descendant. Uncertain effects are reconciled, never replayed blind.',
              ].map((text) => (
                <li key={text} className="grid grid-cols-[20px_1fr] gap-2">
                  <Check className="mt-1 size-4 text-leaf" strokeWidth={2} aria-hidden="true" />
                  {text}
                </li>
              ))}
            </ul>
          </div>
        </div>
      </div>
    </section>
  )
}

function OrgCard({ role, id, harness, mark, env, state, lead = false }: { role: string; id: string; harness: string; mark: string; env: string; state: string; lead?: boolean }) {
  return (
    <div className="ticket px-4 py-3" data-state={state === 'In review' ? 'needs' : undefined}>
      <div className="mono flex items-center justify-between text-[12px] text-quiet">
        <span>{lead ? 'LEAD' : 'REPORT'}</span>
        <span>#{id}</span>
      </div>
      <div className="ticket-rule mt-2" />
      <p className="mt-2 text-[15px] font-semibold text-ink">{role}</p>
      <p className="mt-1 flex items-center gap-1.5 text-[13px] text-ink-2">
        <Mark name={mark} className="size-[13px]" />
        {harness} · {env}
      </p>
      <p className="mt-2 flex items-center gap-1.5 text-[12px] font-medium text-ink-2">
        <span className={`size-1.5 rounded-full ${state === 'Running' || state === 'Delegating' ? 'working-dot bg-leaf' : state === 'In review' ? 'bg-lime' : 'bg-[#b3b8af]'}`} aria-hidden="true" />
        {state}
      </p>
    </div>
  )
}

const REWORK = [
  { id: '#C3', who: 'Engineer', state: 'Superseded', dot: 'bg-[#b3b8af]' },
  { id: 'LEAD', who: 'Verdict', state: 'Rework · round 1 of 2', dot: 'bg-lime' },
  { id: '#C7', who: 'Engineer', state: 'Queued · rework of #C3', dot: 'bg-[#b3b8af]' },
]

function ReviewTrail() {
  return (
    <div className="mt-8 border-t border-dashed border-[#d3d7cf] pt-6">
      <p className="text-[14px] font-semibold text-ink">When the lead sends #C3 back</p>
      <ol className="mt-3 flex flex-col gap-2 md:flex-row md:items-stretch">
        {REWORK.map((step, index) => (
          <li key={step.id} className="flex flex-col gap-2 md:flex-1 md:flex-row md:items-center">
            {index > 0 && <ArrowRight className="size-4 shrink-0 rotate-90 self-center text-quiet md:rotate-0" strokeWidth={1.75} aria-hidden="true" />}
            <div className="ticket flex-1 px-3.5 py-2.5">
              <div className="mono flex items-center justify-between text-[12px] text-quiet">
                <span>{step.id}</span>
                <span>{step.who}</span>
              </div>
              <p className="mt-1.5 flex items-center gap-1.5 text-[13px] font-medium text-ink">
                <span className={`size-1.5 shrink-0 rounded-full ${step.dot}`} aria-hidden="true" />
                {step.state}
              </p>
            </div>
          </li>
        ))}
      </ol>
      <p className="mt-3 text-[13px] leading-[1.55] text-ink-2">The replacement goes to the same agent with the findings. After the last allowed round, the agent reports a blocker instead of trying again.</p>
    </div>
  )
}

const STAGES = [
  { n: '01', name: 'Research', team: 'Research crew', out: 'Markdown brief' },
  { n: 'GATE', name: 'Approve the brief', team: 'Decisions', out: 'Your exact approval' },
  { n: '02', name: 'Build', team: 'Engineering', out: 'Code handoff' },
  { n: '03', name: 'Report', team: 'Writers', out: 'PDF' },
]

function Workflows() {
  return (
    <section id="workflows" aria-labelledby="workflows-title" className="border-b border-rule bg-canvas py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="workflows-title" title={<>Workflows that wait at <B>your gate</B></>}>
          Describe a workflow or build it stage by stage. Each stage names its team, instructions and the formats it must produce, and earlier artifacts feed the stages after them.
        </SectionHead>
        <ol className="mt-14 grid grid-cols-1 gap-3 md:grid-cols-4">
          {STAGES.map((stage) => {
            const gate = stage.n === 'GATE'
            return (
              <li key={stage.name} className={`relative flex min-h-[188px] flex-col rounded-[14px] p-5 ${gate ? 'bg-lime text-lime-ink' : 'ticket'}`}>
                <span className={`mono text-[12px] ${gate ? 'text-lime-ink' : 'text-quiet'}`}>{gate ? 'GATE · DECISIONS' : `STAGE ${stage.n}`}</span>
                <h3 className="mt-6 text-[22px] leading-tight font-semibold tracking-[-0.02em]">{stage.name}</h3>
                <dl className={`mt-auto grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 pt-6 text-[13px] ${gate ? 'text-lime-ink' : 'text-ink-2'}`}>
                  <dt className="opacity-70">{gate ? 'Where' : 'Team'}</dt>
                  <dd>{stage.team}</dd>
                  <dt className="opacity-70">{gate ? 'Needs' : 'Produces'}</dt>
                  <dd>{stage.out}</dd>
                </dl>
              </li>
            )
          })}
        </ol>
        <p className="mt-3 text-[13px] text-quiet">Illustration of one workflow definition. Stage names and teams are examples.</p>
        <div className="mt-12 grid grid-cols-1 gap-x-10 gap-y-8 border-t border-ink pt-8 md:grid-cols-3">
          {[
            ['Gates hold the run', 'A stage gate waits in Decisions for your exact approval before the next stage starts. Nothing continues on a timeout.'],
            ['Branches and bounded retries', 'Route on conditions, retry a failed stage within its attempt limit, and keep a code workspace handoff between stages.'],
            ['Routines on a schedule', 'Fire a prompt, team or workflow on an interval. Pause, resume or run now; history records every firing once.'],
          ].map(([title, text]) => (
            <div key={title}>
              <h3 className="text-[17px] font-semibold text-ink">{title}</h3>
              <p className="mt-2 text-[15px] leading-[1.6] text-ink-2">{text}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  )
}

const FILES = [
  ['HTML', 'release-notes-0.1.25.html', '#80F1'],
  ['PDF', 'qa-findings.pdf', '#6E24'],
  ['MD', 'research-brief.md', '#38E2'],
  ['CSV', 'usage-by-harness.csv', '#8CE6'],
  ['SVG', 'pass-figure.svg', '#EA96'],
  ['JSON', 'team-run-limits.json', '#1CC6'],
]

function Artifacts() {
  return (
    <section id="artifacts" aria-labelledby="artifacts-title" className="border-b border-rule py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="artifacts-title" title={<>Every output in one <B>library</B></>}>
          Documents agents produce land in one library, linked to the task and project that made them. Search, filter by project, task or format, and export the exact original.
        </SectionHead>
        <div className="mt-14 grid grid-cols-1 gap-10 lg:grid-cols-12">
          <figure className="lg:col-span-7">
            <div className="rounded-[20px] bg-canvas p-3 sm:p-4">
              <div className="flex flex-wrap items-center gap-2 px-1 pb-3">
                {['All formats', 'HTML', 'PDF', 'Markdown'].map((chip, index) => (
                  <span key={chip} className={`chip min-h-8 text-[12px] ${index === 0 ? 'bg-ink! text-white! shadow-none!' : ''}`}>
                    {chip}
                  </span>
                ))}
              </div>
              <ul className="grid grid-cols-2 gap-3 sm:grid-cols-3">
                {FILES.map(([kind, name, task]) => (
                  <li key={name} className="ticket flex min-h-[132px] flex-col p-3.5">
                    <span className="mono inline-flex w-fit rounded-md bg-canvas px-1.5 py-0.5 text-[11px] font-semibold text-ink">{kind}</span>
                    <span className="mt-3 line-clamp-2 text-[14px] leading-[1.3] font-medium break-words text-ink">{breakable(name)}</span>
                    <span className="mono mt-auto pt-3 text-[12px] text-quiet">task {task}</span>
                  </li>
                ))}
              </ul>
            </div>
            <figcaption className="mt-3 text-[13px] text-quiet">Illustration of the Artifacts library with example files.</figcaption>
          </figure>
          <div className="space-y-8 lg:col-span-5">
            <div>
              <h3 className="text-[17px] font-semibold text-ink">Read it before you trust it</h3>
              <p className="mt-2 text-[15px] leading-[1.6] text-ink-2">
                HTML opens in a verified Reading view or as Source, with no external navigation. PDFs page and zoom. HTML, Markdown, text, CSV, JSON, SVG, XML and YAML keep their originals; PDF, Office and image files are published with a digest of their exact bytes.
              </p>
            </div>
            <div className="border-t border-ink pt-6">
              <h3 className="text-[17px] font-semibold text-ink">Publish static sites</h3>
              <ol className="mt-4 grid grid-cols-3 gap-2">
                {['Build locally', 'Review', 'Upload'].map((step, index) => (
                  <li key={step} className="rounded-[10px] bg-canvas px-3 py-3">
                    <span className="mono block text-[11px] text-quiet">0{index + 1}</span>
                    <span className="mt-1 block text-[14px] font-medium text-ink">{step}</span>
                  </li>
                ))}
              </ol>
              <p className="mt-3 text-[15px] leading-[1.6] text-ink-2">Static only: pages are served through Ion's Cloudflare Worker from private R2 storage, after a local build and your review. No server code runs.</p>
            </div>
          </div>
        </div>
      </div>
    </section>
  )
}

/** File names wrap after a hyphen or before the extension, never inside a word. */
function breakable(name: string) {
  return name.split(/(?<=-)|(?=\.[a-z]+$)/).flatMap((part, index) => (index ? [<wbr key={index} />, part] : [part]))
}

const GRANTS: Array<[string, Array<'grant' | 'ask' | 'none'>]> = [
  ['Lead', ['grant', 'ask', 'none']],
  ['Engineer', ['grant', 'none', 'none']],
  ['Writer', ['none', 'ask', 'ask']],
]

function Apps() {
  return (
    <section aria-labelledby="apps-title" className="border-b border-rule bg-canvas py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="apps-title" title={<>Apps with <B>exact</B> grants</>}>
          Connect accounts through Composio, then grant each agent only the apps, actions and connected accounts it needs. Consequential actions wait for your approval, and every call leaves a receipt.
        </SectionHead>
        <figure className="mt-14">
          <div className="overflow-hidden rounded-[20px] bg-white shadow-[var(--shadow-ticket)]">
            <table className="w-full table-fixed border-collapse text-left">
              <thead>
                <tr className="border-b border-rule text-[12px] tracking-[0.04em] text-quiet uppercase">
                  <th scope="col" className="w-[25%] px-4 py-3 font-medium sm:w-[28%] sm:px-6">Agent</th>
                  {['GitHub · open issue', 'Slack · post message', 'Gmail · send draft'].map((action) => (
                    <th key={action} scope="col" className="px-2 py-3 font-medium sm:px-4">
                      {action}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {GRANTS.map(([agent, cells]) => (
                  <tr key={agent} className="border-b border-rule last:border-b-0">
                    <th scope="row" className="px-4 py-4 text-[15px] font-semibold text-ink sm:px-6">
                      {agent}
                    </th>
                    {cells.map((cell, index) => (
                      <td key={index} className="px-2 py-4 sm:px-4">
                        <div className="flex min-h-7 items-center">
                          {cell === 'grant' ? (
                            <span className="inline-flex items-center gap-1.5 text-[13px] font-medium text-leaf">
                              <Check className="size-4" strokeWidth={2} aria-hidden="true" />
                              Granted
                            </span>
                          ) : cell === 'ask' ? (
                            <span className="inline-flex items-center rounded-full bg-lime px-2 py-0.5 text-[12px] font-semibold whitespace-nowrap text-lime-ink">
                              Approve<span className="hidden sm:inline">&nbsp;each</span>
                            </span>
                          ) : (
                            <span className="text-[13px] whitespace-nowrap text-[#9aa197]">No access</span>
                          )}
                        </div>
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <figcaption className="mt-3 text-[13px] text-quiet">Illustration of per-agent grants. Available apps and actions come from your live Composio catalog.</figcaption>
        </figure>
      </div>
    </section>
  )
}

function SpeechAndPhone() {
  return (
    <section aria-label="Local speech and the Android companion" className="border-b border-rule">
      <div className={`${container} grid grid-cols-1 lg:grid-cols-2`}>
        <div className="border-rule py-20 sm:py-24 lg:border-r lg:pr-12">
          <h2 className="display display-md">
            Dictate, <B>locally</B>
          </h2>
          <p className="mt-6 max-w-[48ch] text-[17px] leading-[1.6] text-ink-2">
            Press the mic, speak, press Stop. NVIDIA Parakeet runs on your desktop's CPU through NeMo-Speech.cpp, downloaded once and checked against its digest. Desktop audio stays on the computer.
          </p>
          <dl className="mt-8 grid grid-cols-[auto_1fr] gap-x-6 border-t border-ink text-[15px]">
            {[
              ['Model', 'parakeet-tdt_ctc-110m, Q8'],
              ['Download', '146 MB, SHA-256 verified'],
              ['Language', 'English, with punctuation'],
              ['Phone', 'Transcribed on the paired desktop'],
            ].map(([term, value]) => (
              <div key={term} className="col-span-2 grid grid-cols-subgrid border-b border-rule py-3">
                <dt className="text-quiet">{term}</dt>
                <dd className="mono text-[14px] text-ink">{value}</dd>
              </div>
            ))}
          </dl>
        </div>
        <div className="grid grid-cols-1 gap-10 py-20 sm:grid-cols-[1fr_auto] sm:py-24 lg:pl-12">
          <div>
            <h2 className="display display-md">
              The pass in your <B>pocket</B>
            </h2>
            <p className="mt-6 max-w-[44ch] text-[17px] leading-[1.6] text-ink-2">
              The Android companion pairs with your desktop daemon or personal gateway. Read tasks and artifacts, answer decisions, and start, steer, stop or decline work on any desktop harness, Nebius included, from the phone.
            </p>
            <p className="mt-4 max-w-[44ch] text-[15px] leading-[1.6] text-quiet">Workflow stage approvals and Nebius stage generation stay on the desktop.</p>
          </div>
          <Phone />
        </div>
      </div>
    </section>
  )
}

function Phone() {
  return (
    <div aria-hidden="true" className="mx-auto w-[220px] rounded-[34px] bg-ink p-[9px] shadow-[var(--shadow-float)]">
      <div className="h-[400px] overflow-hidden rounded-[25px] bg-canvas px-3 pt-4">
        <div className="flex items-center justify-between px-1">
          <span className="text-[13px] font-semibold text-ink">Work</span>
          <span className="flex size-6 items-center justify-center rounded-[7px] bg-lime text-lime-ink">
            <span className="ion-mark size-3.5" />
          </span>
        </div>
        <div className="mt-3 space-y-2">
          {[
            ['80F1', 'Draft onboarding copy', 'needs'],
            ['8CE6', 'Theme toggle in settings', 'working'],
            ['6E24', 'QA findings as PDF', 'ready'],
            ['38E2', 'Rename sidebar section', 'ready'],
          ].map(([id, title, stage]) => (
            <div key={id} className="ticket rounded-[11px] px-3 py-2.5" data-state={stage}>
              <div className="mono flex justify-between text-[10px] text-quiet">
                <span>#{id}</span>
                <span>{stage === 'needs' ? 'Needs you' : stage === 'working' ? 'Working' : 'Ready'}</span>
              </div>
              <p className="mt-1 text-[12px] font-medium text-ink">{title}</p>
              {stage === 'needs' ? (
                <div className="mt-2 grid grid-cols-2 gap-1.5">
                  <span className="rounded-md bg-lime py-1 text-center text-[11px] font-semibold text-lime-ink">Approve</span>
                  <span className="rounded-md bg-[#eef0eb] py-1 text-center text-[11px] font-semibold text-ink">Decline</span>
                </div>
              ) : null}
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function Nebius() {
  return (
    <section id="nebius" aria-labelledby="nebius-title" className="ion-night bg-night py-20 text-night-text sm:py-28">
      <div className={container}>
        <div className="grid grid-cols-1 gap-12 lg:grid-cols-12 lg:items-end">
          <h2 id="nebius-title" className="display display-lg lg:col-span-7">
            Bring your own <B>Token Factory</B> key
          </h2>
          <p className="max-w-[46ch] text-[17px] leading-[1.6] text-night-quiet lg:col-span-5 lg:justify-self-end">
            Save a Nebius Token Factory key under Settings, test it against the live model catalog, and pick Nebius per task or per agent. Ion is independent of Nebius; you bring the key or use an operator-managed service your account can reach.
          </p>
        </div>
        <div className="mt-14 grid grid-cols-1 gap-10 lg:grid-cols-12">
          <div className="rounded-[20px] bg-night-2 p-5 shadow-[var(--shadow-night)] sm:p-7 lg:col-span-6 lg:self-start">
            <p className="text-[13px] tracking-[0.04em] text-night-quiet uppercase">Settings · Models &amp; Speech</p>
            <p className="mt-4 text-[17px] font-semibold">Nebius Token Factory</p>
            <div className="mt-4 flex flex-wrap items-center gap-2">
              <span className="mono flex min-h-11 flex-1 items-center gap-2 rounded-[10px] bg-night px-3 text-[14px] text-night-quiet shadow-[var(--shadow-night)]">
                <KeyRound className="size-4" strokeWidth={1.75} aria-hidden="true" />
                ••••••••••••••••
              </span>
              <span className="inline-flex min-h-11 items-center rounded-[10px] bg-lime px-4 text-[14px] font-semibold text-lime-ink">Test</span>
            </div>
            <ul className="mt-5 space-y-2 text-[14px]">
              <li className="flex items-start gap-2">
                <Lock className="mt-0.5 size-4 shrink-0 text-lime" strokeWidth={1.75} aria-hidden="true" />
                <span>Stored in OS vault</span>
              </li>
              <li className="flex items-start gap-2">
                <ShieldCheck className="mt-0.5 size-4 shrink-0 text-lime" strokeWidth={1.75} aria-hidden="true" />
                <span>
                  Model ID checked against <span className="mono text-[13px] whitespace-nowrap">GET /v1/models</span> on every start
                </span>
              </li>
            </ul>
            <p className="mt-5 text-[12px] text-night-quiet">Illustration of the settings panel.</p>
          </div>
          <dl className="grid grid-cols-1 content-start gap-y-6 lg:col-span-6">
            {[
              ['Model', 'A daemon-owned driver prefers NVIDIA Nemotron 3 Super when your catalog lists it.'],
              ['Tools', 'List files, read files, and replace a file after a one-time approval that shows the full new content. No arbitrary shell.'],
              ['Recovery', 'Each write is journaled before it touches a file, so a resumed task never applies it twice.'],
              ['Teams', 'Every agent picks Local or Nebius sandbox on its own; a local fallback stays visible.'],
            ].map(([term, text]) => (
              <div key={term} className="grid grid-cols-1 gap-2 border-t border-night-rule pt-5 sm:grid-cols-[140px_1fr]">
                <dt className="mono text-[13px] tracking-[0.04em] text-lime uppercase">{term}</dt>
                <dd className="text-[16px] leading-[1.6] text-night-text">{text}</dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </section>
  )
}

function Downloads({ publishedCount }: { publishedCount: number }) {
  const [copied, setCopied] = useState<string | null>(null)
  const copy = (sha: string) => {
    void navigator.clipboard.writeText(sha).then(() => {
      setCopied(sha)
      setTimeout(() => setCopied(null), 2400)
    })
  }
  return (
    <section id="downloads" aria-labelledby="downloads-title" className="border-b border-rule bg-canvas py-20 sm:py-28">
      <div className={container}>
        <SectionHead id="downloads-title" title={<>Get <B>Ion</B></>}>
          {publishedCount
            ? 'Check the version, size and SHA-256 before you install. A platform without a published build stays unavailable.'
            : 'These are the verified local builds. Public hosting is not configured yet, so the download buttons switch on once a release URL is published.'}
        </SectionHead>
        <div className="mt-14 grid grid-cols-1 gap-4 md:grid-cols-2">
          {releases.map((release) => {
            const windows = release.platform === 'windows'
            const Icon = windows ? Monitor : Smartphone
            return (
              <article key={release.platform} className="ticket flex flex-col overflow-hidden rounded-[20px]">
                <div className="flex items-start justify-between gap-4 p-6 sm:p-7">
                  <div className="flex items-center gap-3">
                    <span className="flex size-11 items-center justify-center rounded-[12px] bg-canvas text-ink">
                      <Icon className="size-5" strokeWidth={1.75} aria-hidden="true" />
                    </span>
                    <div>
                      <h3 className="text-[22px] font-semibold tracking-[-0.02em] text-ink">{release.name}</h3>
                      <p className="text-[14px] text-ink-2">{windows ? 'Desktop IDE' : 'Companion app'}</p>
                    </div>
                  </div>
                  <span className="mono rounded-full bg-canvas px-2.5 py-1 text-[13px] text-ink">v{release.version}</span>
                </div>
                <dl className="mono grid grid-cols-1 border-y border-dashed border-rule text-[13px] sm:grid-cols-3">
                  {[
                    ['Format', release.format],
                    ['Arch', release.architecture],
                    ['Size', formatSize(release.bytes)],
                  ].map(([term, value]) => (
                    <div key={term} className="flex items-baseline justify-between gap-4 border-b border-dashed border-rule px-6 py-2.5 last:border-b-0 sm:block sm:border-r sm:border-b-0 sm:px-7 sm:py-3 sm:last:border-r-0">
                      <dt className="text-[11px] tracking-[0.06em] text-quiet uppercase">{term}</dt>
                      <dd className="text-right text-ink sm:mt-1 sm:text-left">{value}</dd>
                    </div>
                  ))}
                </dl>
                <div className="flex flex-1 flex-col p-6 sm:p-7">
                  <p className="text-[15px] leading-[1.6] text-ink-2">{release.notes}</p>
                  <div className="mt-auto pt-6">
                    <div className="flex items-center justify-between gap-3">
                      <span className="text-[12px] tracking-[0.06em] text-quiet uppercase">SHA-256</span>
                      <button
                        type="button"
                        onClick={() => copy(release.sha256)}
                        className="inline-flex min-h-9 items-center gap-1.5 rounded-lg px-2 text-[13px] font-medium text-leaf transition-[scale,background-color] duration-150 ease-out active:scale-[0.96] [@media(hover:hover)]:hover:bg-[#e9f3dc]"
                      >
                        {copied === release.sha256 ? <Check className="size-4" strokeWidth={2} aria-hidden="true" /> : <Copy className="size-4" strokeWidth={1.75} aria-hidden="true" />}
                        <span aria-live="polite">{copied === release.sha256 ? 'Copied' : 'Copy'}</span>
                      </button>
                    </div>
                    <code className="mono mt-1.5 block rounded-[10px] bg-canvas px-3 py-2.5 text-[12px] leading-[1.5] break-all text-ink">{release.sha256}</code>
                  </div>
                  <div className="mt-6">
                    {release.url ? (
                      <a href={release.url} className="block-link w-full" data-tone="lime">
                        <span className="inline-flex items-center gap-2">
                          <Download className="size-[18px]" strokeWidth={2} aria-hidden="true" />
                          Download {windows ? 'Windows installer' : 'Android APK'}
                        </span>
                        <ArrowUpRight className="size-[18px]" strokeWidth={2} aria-hidden="true" />
                      </a>
                    ) : (
                      <>
                        <button type="button" disabled className="block-link w-full cursor-not-allowed bg-[#e9ebe6] text-[#7d847a]">
                          <span className="inline-flex items-center gap-2">
                            <Download className="size-[18px]" strokeWidth={2} aria-hidden="true" />
                            {windows ? 'Installer' : 'APK'} not published yet
                          </span>
                        </button>
                        <p className="mt-2 text-[13px] text-quiet">Verified on disk. Waiting for a public HTTPS release URL.</p>
                      </>
                    )}
                  </div>
                </div>
              </article>
            )
          })}
        </div>
      </div>
    </section>
  )
}

const QUESTIONS: Array<[string, string]> = [
  ['Do I need an account?', 'Yes. Sign in with your approved Ion account on desktop or mobile. Local tasks then run through the agent CLIs installed and authenticated on your own computer.'],
  [
    'Which agents does Ion run?',
    'Ion detects Amp, Claude Code, Codex CLI, Cursor CLI, Fx, Grok Build, Kimi Code, OpenCode and Pi, and runs Nebius Token Factory with your key. Execution modes vary by adapter, and team delegation is not finished for every harness yet.',
  ],
  [
    'How does Nebius fit in?',
    'Token Factory is an optional model provider. Save and test your own key in Settings, or use an operator-managed service where your account has access, and choose Nebius per task or per agent. Nebius cloud features need a deployed Ion gateway and enabled operator policy. Ion is not a Nebius product.',
  ],
  ['What does the Android app connect to?', 'Your paired desktop daemon or personal gateway. It needs a reachable endpoint and authentication; it is a companion, not a standalone IDE. Phone dictation is transcribed on that desktop.'],
  ['How do I install Ion on Android?', 'Download the latest APK from this page, then open it in your phone’s Files app. Allow installation from that source if Android asks. Install it over your existing Ion app to keep its settings. The download is for ARM64 phones running Android 7.0 or newer.'],
  ['Where does my work go?', 'Projects, transcripts and task data stay with the Ion daemon on your machine. Requests go to the agent providers, apps and Nebius endpoints you configure, under your credentials. Desktop dictation runs locally.'],
  ['Can Ion publish what agents make?', "Static HTML only: after a local build and your review, pages are served through Ion's Cloudflare Worker from private R2 storage. No server code is deployed."],
  ['Why can’t I download Ion yet?', 'The Windows and Android builds are verified locally, but no public download URL is published. The buttons switch on as soon as a verified HTTPS release URL is configured.'],
]

function Faq({ publishedCount }: { publishedCount: number }) {
  const shown = QUESTIONS.filter(([question]) => !publishedCount || !question.startsWith('Why can'))
  return (
    <section id="faq" aria-labelledby="faq-title" className="py-20 sm:py-28">
      <div className={`${container} grid grid-cols-1 gap-12 lg:grid-cols-12`}>
        <h2 id="faq-title" className="display display-lg lg:col-span-5">
          Questions, <B>answered</B>
        </h2>
        <Accordion className="border-t border-ink lg:col-span-7">
          {shown.map(([question, answer]) => (
            <AccordionItem key={question} value={question} className="border-b border-rule">
              <AccordionTrigger className="min-h-14 items-center py-4 text-[17px] font-semibold text-ink hover:no-underline">{question}</AccordionTrigger>
              <AccordionContent className="pb-5 text-[16px] leading-[1.6] text-ink-2">{answer}</AccordionContent>
            </AccordionItem>
          ))}
        </Accordion>
      </div>
    </section>
  )
}

function Footer() {
  return (
    <footer className="ion-night bg-night text-night-text">
      <div className={`${container} pt-20`}>
        <div className="grid grid-cols-1 gap-12 md:grid-cols-12">
          <div className="md:col-span-5">
            <p className="max-w-[36ch] text-[17px] leading-[1.6] text-night-quiet">A native desktop IDE and Android companion for coordinating coding agents, teams, approvals and budgets. Built in Rust with GPUI.</p>
          </div>
          <nav aria-label="Footer" className="grid grid-cols-2 gap-8 text-[14px] md:col-span-4">
            <ul className="space-y-1">
              {NAV.slice(0, 4).map(([label, href]) => (
                <li key={href}>
                  <a href={href} className="inline-flex min-h-10 items-center text-night-text [@media(hover:hover)]:hover:text-lime">
                    {label}
                  </a>
                </li>
              ))}
            </ul>
            <ul className="space-y-1">
              {NAV.slice(4).map(([label, href]) => (
                <li key={href}>
                  <a href={href} className="inline-flex min-h-10 items-center text-night-text [@media(hover:hover)]:hover:text-lime">
                    {label}
                  </a>
                </li>
              ))}
            </ul>
          </nav>
          <div className="md:col-span-3 md:justify-self-end">
            <a href="#downloads" className="block-link min-h-[120px] w-full min-w-[220px] flex-col items-start justify-between py-4" data-tone="lime">
              <ArrowUpRight className="size-5 self-end" strokeWidth={2} aria-hidden="true" />
              <span>Get Ion</span>
            </a>
          </div>
        </div>
        <div className="@container mt-20">
          <p aria-hidden="true" className="display pb-[0.1em] text-[length:calc(100cqw/2.4)] leading-[0.8] tracking-[-0.06em] whitespace-nowrap select-none">
            <span className="text-lime">[</span>ION<span className="text-lime">]</span>
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-night-rule py-6 text-[13px] text-night-quiet">
          <span>© 2026 Ion · GPL-3.0</span>
          <span>Windows desktop · Android companion</span>
        </div>
      </div>
    </footer>
  )
}
