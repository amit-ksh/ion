import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { motion } from 'framer-motion'
import {
  ArrowDown,
  ArrowRight,
  ArrowUpRight,
  Check,
  ChevronDown,
  CircleCheck,
  Copy,
  Cpu,
  Download,
  ExternalLink,
  FileCode,
  FileText,
  Folder,
  GitBranch,
  Layers,
  Lock,
  Monitor,
  Network,
  Play,
  RotateCcw,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Terminal,
  Workflow
} from 'lucide-react'
import { releases, formatSize } from '@/lib/release'
import { Accordion, AccordionContent, AccordionItem, AccordionTrigger } from '@/components/ui/accordion'
import { AppWindow } from '@/components/app-window'

export const Route = createFileRoute('/')({ component: Home })

const bentoFeatures = [
  {
    tag: 'MODEL_AGNOSTIC',
    title: 'Nebius Model Integration',
    description:
      'High-throughput inference with open-weight models powered by Nebius AI Studio, including Meta-Llama 3.1 405B and Qwen 2.5 Coder 32B. Bring your own keys with zero markup and localized secret storage.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M50 15 L85 35 L85 70 L50 90 L15 70 L15 35 Z" strokeDasharray="3 3" className="stroke-slate-300" />
        <path d="M50 15 L50 50 L85 35" />
        <path d="M50 50 L15 35" />
        <path d="M50 50 L50 90" />
        <rect x="38" y="32" width="24" height="24" rx="4" className="stroke-[#65a30d] fill-[#c4f25c]/20" />
        <circle cx="50" cy="44" r="3" className="fill-[#65a30d]" />
      </svg>
    ),
  },
  {
    tag: 'ENTERPRISE_GRADE',
    title: 'Local-First Security',
    description:
      'Transcripts, code modifications, and session states stay strictly on your local machine. No mandatory accounts, telemetry or cloud lock-in. Credentials never leave your device without explicit approval.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <path d="M50 15 L80 28 L80 55 C80 72 50 88 50 88 C50 88 20 72 20 55 L20 28 Z" className="stroke-slate-300" />
        <path d="M50 25 L70 34 L70 53 C70 65 50 78 50 78 C50 78 30 65 30 53 L30 34 Z" className="stroke-[#65a30d] fill-[#c4f25c]/15" />
        <circle cx="50" cy="48" r="6" className="stroke-slate-800 fill-white" />
        <path d="M50 54 L50 62" strokeWidth="2" className="stroke-slate-800" />
      </svg>
    ),
  },
  {
    tag: 'AUTO_REMEDIATION',
    title: 'Hierarchical Agent Teams',
    description:
      'Arrange lead and specialist agents into structured parent–child teams. Bounded task budgets, automated test remediation, linter repair, and strict review gates keep execution disciplined.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="25" y="15" width="50" height="18" rx="4" className="stroke-slate-800 fill-white" />
        <path d="M50 33 L50 48" className="stroke-slate-400" />
        <path d="M30 48 L70 48" className="stroke-slate-400" />
        <path d="M30 48 L30 58" className="stroke-slate-400" />
        <path d="M70 48 L70 58" className="stroke-slate-400" />
        <rect x="15" y="58" width="30" height="20" rx="4" className="stroke-[#65a30d] fill-[#c4f25c]/20" />
        <rect x="55" y="58" width="30" height="20" rx="4" className="stroke-slate-800 fill-white" />
      </svg>
    ),
  },
  {
    tag: 'CODING_HARNESSES',
    title: 'Codex, OpenCode & Pi',
    description:
      'Unify Codex, OpenCode, and Pi in a single high-refresh native GPUI workspace. Follow multi-turn sessions with fluid keyboard navigation and zero latency.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="15" y="20" width="70" height="60" rx="6" className="stroke-slate-300 fill-white" />
        <line x1="15" y1="36" x2="85" y2="36" className="stroke-slate-200" />
        <circle cx="25" cy="28" r="2" className="fill-slate-400" />
        <circle cx="33" cy="28" r="2" className="fill-slate-400" />
        <circle cx="41" cy="28" r="2" className="fill-slate-400" />
        <path d="M28 50 L38 58 L28 66" className="stroke-[#65a30d]" strokeWidth="2" />
        <line x1="44" y1="66" x2="60" y2="66" className="stroke-slate-700" strokeWidth="2" />
      </svg>
    ),
  },
  {
    tag: 'CONNECTED_APPS',
    title: 'Composio Tool Mesh',
    description:
      'Connect accounts through Composio and grant agents specific apps and actions (GitHub, Slack, Notion, Linear). Approvals and action receipts stay directly beside the work.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <circle cx="50" cy="50" r="14" className="stroke-[#65a30d] fill-[#c4f25c]/20" />
        <circle cx="20" cy="30" r="8" className="stroke-slate-400 fill-white" />
        <circle cx="80" cy="30" r="8" className="stroke-slate-400 fill-white" />
        <circle cx="25" cy="75" r="8" className="stroke-slate-400 fill-white" />
        <circle cx="75" cy="75" r="8" className="stroke-slate-400 fill-white" />
        <line x1="27" y1="34" x2="40" y2="44" className="stroke-slate-300" strokeDasharray="2 2" />
        <line x1="73" y1="34" x2="60" y2="44" className="stroke-slate-300" strokeDasharray="2 2" />
        <line x1="32" y1="71" x2="42" y2="59" className="stroke-slate-300" strokeDasharray="2 2" />
        <line x1="68" y1="71" x2="58" y2="59" className="stroke-slate-300" strokeDasharray="2 2" />
      </svg>
    ),
  },
  {
    tag: 'DOCUMENT_ARTIFACTS',
    title: 'Live Artifact Previews',
    description:
      'Browse artifacts by task, project or format. Read HTML and PDF previews, inspect code diffs and export original files including technical specs and spreadsheets.',
    graphic: (
      <svg className="w-28 h-28 text-slate-800" viewBox="0 0 100 100" fill="none" stroke="currentColor" strokeWidth="1.5">
        <rect x="25" y="18" width="50" height="64" rx="4" className="stroke-slate-300 fill-white" />
        <path d="M55 18 L75 38 L55 38 Z" className="stroke-slate-300 fill-slate-100" />
        <line x1="35" y1="46" x2="65" y2="46" className="stroke-[#65a30d]" strokeWidth="2" />
        <line x1="35" y1="56" x2="65" y2="56" className="stroke-slate-300" strokeWidth="1.5" />
        <line x1="35" y1="66" x2="55" y2="66" className="stroke-slate-300" strokeWidth="1.5" />
      </svg>
    ),
  },
]

const questions = [
  [
    'What do I need to get started?',
    'Install a published Ion build when one is available, then configure your coding harnesses and their authentication. Connected apps, Nebius model access, speech and remote access each need their own configuration in your local workspace.',
  ],
  [
    'How does Nebius fit into Ion?',
    'Ion includes native configuration for Nebius model access and cloud-related workflows (such as Meta-Llama 3.1 405B and Qwen 2.5 Coder). Model credentials and any remote execution hosts are configured directly by the user. This integration does not imply a Nebius partnership, bundled compute, or an available hosted Ion service.',
  ],
  [
    'What does the Android app connect to?',
    'The Android mobile companion connects to your configured desktop daemon or personal gateway for remote tasks, human approval decisions, and artifact review. A reachable endpoint and authenticated pairing are required; it is a companion, not a standalone desktop compiler.',
  ],
  [
    'Why are the downloads unavailable?',
    'Local Windows (v0.1.24) and Android (v1.0.0) builds have been compiled and verified on disk, but no public artifact URL has been published yet. The release details below reflect those verified local builds. Download buttons activate automatically as soon as verified HTTPS release URLs are configured.',
  ],
  [
    'Where does my work go?',
    'Ion stores desktop project state, session histories, and diffs strictly on your local machine. Requests sent to coding providers, connected apps, or a configured Nebius endpoint are handled directly by those services according to the credentials you provide.',
  ],
]

function Home() {
  const publishedCount = releases.filter((release) => release.url).length
  const [copiedSha, setCopiedSha] = useState<string | null>(null)

  const copyToClipboard = (sha: string) => {
    navigator.clipboard.writeText(sha)
    setCopiedSha(sha)
    setTimeout(() => setCopiedSha(null), 2500)
  }

  return (
    <div className="ion-site min-h-dvh bg-white text-slate-900 antialiased selection:bg-[#c4f25c] selection:text-[#141f0c]">
      <a className="skip-link" href="#main">
        Skip to content
      </a>

      {/* Subtle Dot Grid Background in Hero Area */}
      <div className="relative overflow-hidden">
        <div
          aria-hidden="true"
          className="pointer-events-none absolute inset-0 -z-10 dot-grid opacity-35"
        />
        <div
          aria-hidden="true"
          className="pointer-events-none absolute top-0 left-1/2 -z-10 h-[650px] w-full max-w-7xl -translate-x-1/2 bg-[radial-gradient(ellipse_at_top,_rgba(196,242,92,0.18)_0%,_rgba(255,255,255,0)_70%)]"
        />

        {/* Global Navigation Bar */}
        <header className="sticky top-0 z-40 border-b border-slate-200/80 bg-white/85 backdrop-blur-md">
          <div className="mx-auto flex h-16 max-w-7xl items-center justify-between px-5 md:px-10">
            {/* Brand Logo & Nebius lockup */}
            <div className="flex items-center gap-3">
              <a href="/" aria-label="Ion home" className="group flex items-center gap-2.5">
                <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-lime-100/90 border border-lime-300 text-[#65a30d] shadow-2xs transition-transform group-hover:scale-105">
                  <span className="ion-mark size-5" aria-hidden="true" />
                </span>
                <div className="flex flex-col">
                  <span className="text-xl font-bold tracking-tight text-slate-950 lowercase leading-none">ion</span>
                  <span className="mt-1 flex items-center gap-1 text-[11px] font-medium text-slate-500 leading-none">
                    powered by <span className="font-bold text-[#65a30d]">Nebius</span>
                    <span className="size-1 rounded-full bg-[#65a30d]" />
                  </span>
                </div>
              </a>
            </div>

            {/* Navigation Links */}
            <nav aria-label="Main navigation" className="hidden md:flex items-center gap-6 text-sm font-medium text-slate-600">
              <a className="transition-colors hover:text-slate-950" href="#features">Features</a>
              <a className="transition-colors hover:text-slate-950" href="#architecture">Architecture</a>
              <a className="transition-colors hover:text-slate-950" href="#nebius">Nebius Model</a>
              <a className="transition-colors hover:text-slate-950" href="#downloads">Downloads</a>
              <a className="transition-colors hover:text-slate-950" href="#faq">FAQ</a>
            </nav>

            {/* Quick Header CTA */}
            <div className="flex items-center gap-3">
              <a
                href="#downloads"
                className="inline-flex items-center gap-2 rounded-lg bg-[#c4f25c] px-4 py-2 text-xs font-semibold text-[#141f0c] shadow-sm transition-all hover:bg-[#b4e34a] hover:shadow-md focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#65a30d]"
              >
                <Download className="size-3.5" />
                <span>Download App</span>
              </a>
            </div>
          </div>
        </header>

        {/* HERO SECTION (Faithful to layout in reference image with Ion/Nebius branding) */}
        <main id="main">
          <section className="relative px-5 pt-16 pb-12 md:px-10 md:pt-24 md:pb-20">
            <div className="mx-auto max-w-4xl text-center">
              {/* Capsule Pill Badge */}
              <motion.div
                initial={{ opacity: 0, y: -12 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
                className="inline-flex items-center gap-2 rounded-full border border-lime-300/90 bg-lime-50/90 px-3.5 py-1 text-xs font-semibold text-lime-950 shadow-xs"
              >
                <Sparkles className="size-3.5 text-[#65a30d]" />
                <span>High-Throughput Agentic Harness • Nebius AI Studio</span>
              </motion.div>

              {/* Bold Punchy Headline (Ship Features, Not Just Code style) */}
              <motion.h1
                initial={{ opacity: 0, y: 15 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.6, delay: 0.1 }}
                className="mt-6 text-5xl font-extrabold tracking-tight text-slate-950 sm:text-6xl md:text-7xl leading-[1.05]"
              >
                Ship Features, <br className="hidden sm:inline" />
                <span className="text-slate-900">Not Just Code.</span>
              </motion.h1>

              {/* Subtitle */}
              <motion.p
                initial={{ opacity: 0, y: 15 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.6, delay: 0.2 }}
                className="mx-auto mt-6 max-w-2xl text-lg text-slate-600 leading-relaxed md:text-xl"
              >
                The native agentic workspace that coordinates hierarchical agent teams, coding harnesses, connected apps, and document workflows — powered by high-throughput Nebius AI models.
              </motion.p>

              {/* Dual Action Download Buttons */}
              <motion.div
                initial={{ opacity: 0, y: 15 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.6, delay: 0.3 }}
                className="mt-9 flex flex-wrap items-center justify-center gap-4"
              >
                <a
                  href="#downloads"
                  className="inline-flex items-center justify-center gap-2.5 rounded-xl bg-[#c4f25c] px-6 py-3.5 text-sm font-semibold text-[#141f0c] shadow-[0_10px_25px_-5px_rgba(196,242,92,0.5)] transition-all hover:bg-[#b4e34a] hover:scale-[1.02] active:scale-[0.98] focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#65a30d]"
                >
                  <Monitor className="size-4" />
                  <span>Download for Windows</span>
                  <span className="rounded bg-black/10 px-1.5 py-0.5 text-[11px] font-mono">v0.1.24</span>
                </a>

                <a
                  href="#downloads"
                  className="inline-flex items-center justify-center gap-2 rounded-xl border border-slate-300 bg-white px-5 py-3.5 text-sm font-semibold text-slate-800 shadow-xs transition-all hover:bg-slate-50 hover:border-slate-400 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[#65a30d]"
                >
                  <Smartphone className="size-4 text-slate-600" />
                  <span>Android APK</span>
                  <span className="text-xs text-slate-500 font-normal">v1.0.0</span>
                </a>

                <a
                  href="#features"
                  className="inline-flex items-center justify-center gap-1.5 px-4 py-3.5 text-sm font-medium text-slate-600 transition-colors hover:text-slate-950"
                >
                  <span>Explore Features</span>
                  <ArrowRight className="size-4" />
                </a>
              </motion.div>

              {/* Verified Availability Note */}
              <motion.p
                initial={{ opacity: 0 }}
                animate={{ opacity: 1 }}
                transition={{ duration: 0.6, delay: 0.4 }}
                className="mt-4 text-xs text-slate-500 font-mono"
              >
                Windows x86_64 installer · Android ARM64 companion · {publishedCount ? 'Published releases available' : 'Local verified builds'}
              </motion.p>
            </div>

            {/* Interactive Showcase App Window (Centerpiece with floating QA window) */}
            <div className="mt-14 md:mt-20">
              <AppWindow />
            </div>
          </section>

          {/* SUPPORTED HARNESSES & MODEL INTEGRATIONS (Logo Cloud) */}
          <section className="border-y border-slate-200/80 bg-slate-50/60 py-10">
            <div className="mx-auto max-w-7xl px-5 text-center md:px-10">
              <p className="text-xs font-semibold tracking-wider text-slate-500 uppercase font-mono">
                Coordinated Harnesses & Model Integrations
              </p>
              <div className="mt-6 flex flex-wrap items-center justify-center gap-3 sm:gap-4">
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <span className="size-2 rounded-full bg-[#65a30d]" />
                  Nebius AI Studio
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <Cpu className="size-3.5 text-slate-600" />
                  Meta Llama 3.1 405B
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <Terminal className="size-3.5 text-slate-600" />
                  Qwen 2.5 Coder 32B
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <Layers className="size-3.5 text-slate-600" />
                  Codex Harness
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <FileCode className="size-3.5 text-slate-600" />
                  OpenCode
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <Sparkles className="size-3.5 text-slate-600" />
                  Pi Engine
                </span>
                <span className="inline-flex items-center gap-2 rounded-full border border-slate-200 bg-white px-4 py-2 text-xs font-medium text-slate-800 shadow-2xs">
                  <Network className="size-3.5 text-slate-600" />
                  Composio Connected Apps
                </span>
              </div>
            </div>
          </section>

          {/* BENTO FEATURE GRID (Mirroring the cards on the right side of the screenshot) */}
          <section id="features" className="px-5 py-20 md:px-10 md:py-28" aria-labelledby="features-heading">
            <div className="mx-auto max-w-7xl">
              <div className="max-w-2xl">
                <span className="rounded-full bg-lime-100 px-3 py-1 text-xs font-semibold text-lime-900 font-mono">
                  CORE CAPABILITIES
                </span>
                <h2 id="features-heading" className="mt-4 text-3xl font-bold tracking-tight text-slate-950 sm:text-4xl">
                  Engineered for depth. Built for developers.
                </h2>
                <p className="mt-3 text-base text-slate-600 leading-relaxed">
                  Ion replaces fragmented chat interfaces with a native desktop workspace where lead architects and specialist agents work in concert.
                </p>
              </div>

              {/* 3-Column Bento Grid */}
              <div className="mt-12 grid grid-cols-1 gap-6 md:grid-cols-2 lg:grid-cols-3">
                {bentoFeatures.map((feature, i) => (
                  <motion.div
                    key={feature.tag}
                    initial={{ opacity: 0, y: 20 }}
                    whileInView={{ opacity: 1, y: 0 }}
                    viewport={{ once: true }}
                    transition={{ duration: 0.5, delay: i * 0.1 }}
                    className="group relative flex flex-col justify-between overflow-hidden rounded-2xl border border-slate-200 bg-white p-7 shadow-xs transition-all hover:border-[#65a30d]/50 hover:shadow-lg"
                  >
                    <div>
                      <div className="flex items-center justify-between">
                        <span className="font-mono text-[11px] font-bold tracking-wider text-[#65a30d] uppercase">
                          {feature.tag}
                        </span>
                        <div className="transition-transform duration-300 group-hover:scale-110">
                          {feature.graphic}
                        </div>
                      </div>

                      <h3 className="mt-4 text-xl font-bold tracking-tight text-slate-900">
                        {feature.title}
                      </h3>
                      <p className="mt-2.5 text-sm text-slate-600 leading-relaxed">
                        {feature.description}
                      </p>
                    </div>

                    <div className="mt-6 flex items-center gap-1.5 text-xs font-semibold text-slate-900 group-hover:text-[#65a30d]">
                      <span>Learn more</span>
                      <ArrowRight className="size-3.5 transition-transform group-hover:translate-x-1" />
                    </div>
                  </motion.div>
                ))}
              </div>
            </div>
          </section>

          {/* WORKFLOW SEQUENCE SECTION */}
          <section id="architecture" className="border-y border-slate-200/80 bg-slate-50/50 px-5 py-20 md:px-10" aria-labelledby="work-heading">
            <div className="mx-auto max-w-7xl">
              <div className="flex flex-wrap items-baseline justify-between gap-4">
                <div>
                  <span className="text-xs font-semibold tracking-wider text-[#65a30d] uppercase font-mono">
                    LIFECYCLE ARCHITECTURE
                  </span>
                  <h2 id="work-heading" className="mt-2 text-3xl font-bold tracking-tight text-slate-950">
                    From raw spec to reviewed result
                  </h2>
                </div>
                <p className="text-sm text-slate-500 font-mono">Continuous hierarchical execution</p>
              </div>

              <div className="mt-12 grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-4">
                <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-2xs">
                  <div className="flex size-10 items-center justify-center rounded-lg bg-lime-50 text-[#65a30d]">
                    <GitBranch className="size-5" />
                  </div>
                  <span className="mt-4 block font-mono text-xs font-semibold text-slate-400">01 / DELEGATE</span>
                  <h3 className="mt-1 text-lg font-bold text-slate-900">Task Decomposition</h3>
                  <p className="mt-2 text-sm text-slate-600 leading-relaxed">
                    A lead architect decomposes requirements into bounded research, code, and test goals assigned to specialist agents.
                  </p>
                </div>

                <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-2xs">
                  <div className="flex size-10 items-center justify-center rounded-lg bg-lime-50 text-[#65a30d]">
                    <Terminal className="size-5" />
                  </div>
                  <span className="mt-4 block font-mono text-xs font-semibold text-slate-400">02 / EXECUTE</span>
                  <h3 className="mt-1 text-lg font-bold text-slate-900">Multi-Harness Run</h3>
                  <p className="mt-2 text-sm text-slate-600 leading-relaxed">
                    Specialists execute in parallel using Pi and OpenCode, reading files, generating diffs, and requesting tool access.
                  </p>
                </div>

                <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-2xs">
                  <div className="flex size-10 items-center justify-center rounded-lg bg-lime-50 text-[#65a30d]">
                    <Monitor className="size-5" />
                  </div>
                  <span className="mt-4 block font-mono text-xs font-semibold text-slate-400">03 / REVIEW</span>
                  <h3 className="mt-1 text-lg font-bold text-slate-900">Approval Gates</h3>
                  <p className="mt-2 text-sm text-slate-600 leading-relaxed">
                    Inspect results and tool receipts in real time. Approve the next stage, branch the session, or request targeted rework.
                  </p>
                </div>

                <div className="rounded-xl border border-slate-200 bg-white p-6 shadow-2xs">
                  <div className="flex size-10 items-center justify-center rounded-lg bg-lime-50 text-[#65a30d]">
                    <FileText className="size-5" />
                  </div>
                  <span className="mt-4 block font-mono text-xs font-semibold text-slate-400">04 / ARTIFACTS</span>
                  <h3 className="mt-1 text-lg font-bold text-slate-900">Persistent Delivery</h3>
                  <p className="mt-2 text-sm text-slate-600 leading-relaxed">
                    Keep code patches, HTML previews, and PDF documents organized directly with the task that produced them.
                  </p>
                </div>
              </div>
            </div>
          </section>

          {/* NEBIUS MODEL INTEGRATION SECTION */}
          <section id="nebius" className="px-5 py-20 md:px-10" aria-labelledby="nebius-heading">
            <div className="mx-auto max-w-7xl">
              <div className="overflow-hidden rounded-3xl border border-lime-300 bg-gradient-to-br from-lime-50/60 via-white to-white p-8 md:p-12 shadow-sm">
                <div className="grid grid-cols-1 gap-8 lg:grid-cols-2 lg:items-center">
                  <div>
                    <span className="inline-flex items-center gap-2 rounded-full border border-lime-400/80 bg-lime-100/60 px-3 py-1 text-xs font-semibold text-lime-950 font-mono">
                      <Cpu className="size-3.5 text-[#65a30d]" />
                      NEBIUS AI CLOUD & MODEL STACK
                    </span>
                    <h2 id="nebius-heading" className="mt-4 text-3xl font-extrabold tracking-tight text-slate-950 sm:text-4xl">
                      Ultra-fast open weights. Zero markup.
                    </h2>
                    <p className="mt-4 text-base text-slate-600 leading-relaxed">
                      Ion integrates with Nebius AI Studio to power coding agents with high-throughput inference on top-tier open weights like Meta-Llama 3.1 405B and Qwen 2.5 Coder.
                    </p>
                    <p className="mt-3 text-sm text-slate-500 leading-relaxed">
                      Bring your own credentials and configuration. Remote execution requires a separately configured host; a model key alone does not provision infrastructure. Service and model availability depend entirely on your setup.
                    </p>
                    <div className="mt-6 flex flex-wrap gap-4 text-xs font-mono text-slate-700">
                      <span className="flex items-center gap-1.5 rounded-md bg-white border border-slate-200 px-2.5 py-1.5 shadow-2xs">
                        <Check className="size-3.5 text-[#65a30d]" />
                        Meta-Llama-3.1-405B
                      </span>
                      <span className="flex items-center gap-1.5 rounded-md bg-white border border-slate-200 px-2.5 py-1.5 shadow-2xs">
                        <Check className="size-3.5 text-[#65a30d]" />
                        Qwen-2.5-Coder-32B
                      </span>
                      <span className="flex items-center gap-1.5 rounded-md bg-white border border-slate-200 px-2.5 py-1.5 shadow-2xs">
                        <Check className="size-3.5 text-[#65a30d]" />
                        Bring Your Own Key
                      </span>
                    </div>
                  </div>

                  {/* Clean Configuration Preview */}
                  <div className="rounded-2xl border border-slate-700/60 bg-[#121316] p-5 font-mono text-xs text-zinc-300 shadow-xl">
                    <div className="flex items-center justify-between border-b border-white/8 pb-3 text-[11px] text-zinc-400">
                      <span>~/.config/ion/providers.json</span>
                      <span className="text-[#c4f25c]">Nebius AI Endpoint</span>
                    </div>
                    <pre className="mt-3 overflow-x-auto text-zinc-300 leading-relaxed">
                      <code>{`{
  "provider": "nebius",
  "base_url": "https://api.studio.nebius.ai/v1",
  "api_key": "\${NEBIUS_API_KEY}",
  "models": {
    "lead_architect": "meta-llama/Meta-Llama-3.1-405B-Instruct",
    "coder_specialist": "Qwen/Qwen2.5-Coder-32B-Instruct"
  },
  "streaming": {
    "commit_hz": 8.3,
    "temperature": 0.2
  }
}`}</code>
                    </pre>
                  </div>
                </div>
              </div>
            </div>
          </section>

          {/* APPLICATION DOWNLOADS SECTION */}
          <section id="downloads" className="border-t border-slate-200/80 bg-slate-50/70 px-5 py-20 md:px-10" aria-labelledby="downloads-heading">
            <div className="mx-auto max-w-7xl">
              <div className="max-w-2xl">
                <span className="rounded-full bg-lime-100 px-3 py-1 text-xs font-semibold text-lime-900 font-mono">
                  APPLICATION BUILDS
                </span>
                <h2 id="downloads-heading" className="mt-4 text-3xl font-extrabold tracking-tight text-slate-950 sm:text-4xl">
                  Get Ion for Windows & Android
                </h2>
                <p className="mt-3 text-base text-slate-600 leading-relaxed">
                  {publishedCount
                    ? 'Review the version details and build notes before downloading. Platforms without a configured release remain unavailable.'
                    : 'The versions below describe verified local builds. Public download hosting has not been configured yet; download buttons activate when release URLs are set.'}
                </p>
              </div>

              {/* Download Cards Grid */}
              <div className="mt-10 grid grid-cols-1 gap-6 md:grid-cols-2">
                {releases.map((release) => {
                  const isWindows = release.platform === 'windows'
                  return (
                    <article
                      key={release.platform}
                      className="flex flex-col justify-between rounded-2xl border border-slate-200 bg-white p-7 shadow-xs transition-shadow hover:shadow-md"
                    >
                      <div>
                        <div className="flex items-center justify-between">
                          <div className="flex items-center gap-3">
                            <div className="flex size-10 items-center justify-center rounded-xl bg-slate-100 text-slate-800">
                              {isWindows ? <Monitor className="size-5" /> : <Smartphone className="size-5" />}
                            </div>
                            <div>
                              <h3 className="text-xl font-bold text-slate-900">{release.name}</h3>
                              <p className="text-xs text-slate-500 font-mono">
                                {release.format} · {release.architecture}
                              </p>
                            </div>
                          </div>
                          <span className="rounded-full bg-slate-100 px-3 py-1 font-mono text-xs font-semibold text-slate-700">
                            v{release.version}
                          </span>
                        </div>

                        <div className="mt-5 flex items-center gap-4 text-xs font-mono text-slate-600">
                          <span>Size: {formatSize(release.bytes)}</span>
                          <span>•</span>
                          <span>{isWindows ? 'Desktop Workspace' : 'Mobile Companion'}</span>
                        </div>

                        <p className="mt-4 text-sm text-slate-600 leading-relaxed">
                          {release.notes}
                        </p>
                      </div>

                      <div className="mt-8 pt-6 border-t border-slate-100">
                        {release.url ? (
                          <a
                            href={release.url}
                            className="inline-flex w-full items-center justify-center gap-2 rounded-xl bg-[#c4f25c] py-3 text-sm font-semibold text-[#141f0c] shadow-sm transition-all hover:bg-[#b4e34a]"
                          >
                            <Download className="size-4" />
                            <span>Download {isWindows ? 'Windows EXE' : 'Android APK'}</span>
                          </a>
                        ) : (
                          <div>
                            <button
                              disabled
                              className="inline-flex w-full items-center justify-center gap-2 rounded-xl border border-slate-200 bg-slate-100 py-3 text-sm font-semibold text-slate-400 cursor-not-allowed"
                            >
                              <Download className="size-4" />
                              <span>Download {isWindows ? 'EXE' : 'APK'} Unavailable</span>
                            </button>
                            <p className="mt-2 text-center text-xs text-slate-500">
                              Verified local build ready. Awaiting public HTTPS release URL in releases.json.
                            </p>
                          </div>
                        )}

                        {/* Collapsible SHA-256 Checksum */}
                        <div className="mt-5">
                          <div className="flex items-center justify-between text-xs text-slate-500 font-mono">
                            <span>SHA-256 CHECKSUM</span>
                            <button
                              type="button"
                              onClick={() => copyToClipboard(release.sha256)}
                              className="inline-flex items-center gap-1 text-[11px] font-sans text-[#65a30d] hover:underline"
                            >
                              {copiedSha === release.sha256 ? (
                                <>
                                  <Check className="size-3" />
                                  <span>Copied!</span>
                                </>
                              ) : (
                                <>
                                  <Copy className="size-3" />
                                  <span>Copy Hash</span>
                                </>
                              )}
                            </button>
                          </div>
                          <code className="mt-1.5 block rounded-lg bg-slate-50 p-2.5 font-mono text-[10.5px] text-slate-700 break-all border border-slate-200">
                            {release.sha256}
                          </code>
                        </div>
                      </div>
                    </article>
                  )
                })}
              </div>

              {/* Developer Release Configuration Note */}
              <div className="mt-8 rounded-xl border border-slate-200 bg-white p-5 text-xs text-slate-600 leading-relaxed shadow-2xs">
                <span className="font-semibold text-slate-800">Publishing note: </span>
                To configure release downloads, edit <code className="font-mono text-slate-900 bg-slate-100 px-1 py-0.5 rounded">src/lib/releases.json</code> with verified HTTPS URLs and rebuild the site. Checksums and sizes are strictly validated at build time.
              </div>
            </div>
          </section>

          {/* FAQ SECTION */}
          <section id="faq" className="px-5 py-20 md:px-10" aria-labelledby="questions-heading">
            <div className="mx-auto max-w-4xl">
              <span className="rounded-full bg-lime-100 px-3 py-1 text-xs font-semibold text-lime-900 font-mono">
                QUESTIONS & ANSWERS
              </span>
              <h2 id="questions-heading" className="mt-4 text-3xl font-extrabold tracking-tight text-slate-950">
                Frequently Asked Questions
              </h2>
              <Accordion className="mt-8 divide-y divide-slate-200 border-y border-slate-200">
                {questions
                  .filter(([q]) => !publishedCount || q !== 'Why are the downloads unavailable?')
                  .map(([q, a]) => (
                    <AccordionItem key={q} value={q} className="py-4">
                      <AccordionTrigger className="text-base font-semibold text-slate-900 hover:no-underline hover:text-[#65a30d]">
                        {q}
                      </AccordionTrigger>
                      <AccordionContent className="mt-2 text-sm text-slate-600 leading-relaxed">
                        {a}
                      </AccordionContent>
                    </AccordionItem>
                  ))}
              </Accordion>
            </div>
          </section>
        </main>

        {/* MODERN LIGHT FOOTER */}
        <footer className="border-t border-slate-200 bg-slate-50/80 py-12 text-sm text-slate-600">
          <div className="mx-auto flex max-w-7xl flex-wrap items-center justify-between gap-6 px-5 md:px-10">
            <div className="flex items-center gap-3">
              <span className="flex size-8 shrink-0 items-center justify-center rounded-lg bg-lime-100/90 border border-lime-300 text-[#65a30d] shadow-2xs">
                <span className="ion-mark size-5" aria-hidden="true" />
              </span>
              <div className="flex flex-col">
                <div className="flex items-center gap-2 leading-none">
                  <span className="text-lg font-bold tracking-tight text-slate-950 lowercase">ion</span>
                  <span className="text-[11px] font-medium text-slate-400">© 2026</span>
                </div>
                <span className="mt-1 flex items-center gap-1.5 text-xs text-slate-500 leading-none">
                  <span className="size-1.5 rounded-full bg-[#65a30d]" />
                  <span>Powered by <strong className="font-bold text-[#65a30d]">Nebius AI Studio</strong></span>
                </span>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-6 text-xs font-medium">
              <a href="#features" className="hover:text-slate-950">Features</a>
              <a href="#architecture" className="hover:text-slate-950">Architecture</a>
              <a href="#nebius" className="hover:text-slate-950">Nebius Model</a>
              <a href="#downloads" className="hover:text-slate-950">Windows & Android</a>
            </div>

            <a
              href="#downloads"
              className="inline-flex items-center gap-1.5 rounded-lg border border-slate-200 bg-white px-3.5 py-2 text-xs font-semibold text-slate-800 shadow-2xs transition-all hover:bg-slate-50 hover:border-slate-300"
            >
              <span>Get Ion Workspace</span>
              <ArrowUpRight className="size-3.5 text-[#65a30d]" />
            </a>
          </div>
        </footer>
      </div>
    </div>
  )
}
