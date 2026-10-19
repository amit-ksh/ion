import { useState } from 'react'
import { motion, AnimatePresence } from 'framer-motion'
import {
  Terminal,
  CircleCheck,
  Cpu,
  Layers,
  FileCode,
  FileText,
  GitBranch,
  ShieldCheck,
  Sparkles,
  ExternalLink,
  ChevronRight,
  Play,
  RotateCcw,
  Check,
  Folder,
  Workflow,
  Network
} from 'lucide-react'

export function AppWindow() {
  const [activeTab, setActiveTab] = useState<'workflow' | 'diff' | 'artifact'>('workflow')
  const [isRunning, setIsRunning] = useState(false)
  const [sessionStep, setSessionStep] = useState(2)

  const handleSimulate = () => {
    setIsRunning(true)
    setTimeout(() => {
      setSessionStep((prev) => (prev % 3) + 1)
      setIsRunning(false)
    }, 800)
  }

  return (
    <div className="relative mx-auto w-full max-w-5xl select-none">
      {/* Soft Ambient Radial Glow beneath the window */}
      <div 
        aria-hidden="true"
        className="pointer-events-none absolute -inset-4 -z-10 rounded-3xl bg-gradient-to-b from-[#c4f25c]/18 via-[#c4f25c]/6 to-transparent blur-2xl transition-opacity duration-700" 
      />

      {/* Main Desktop IDE Chassis — Dark graphite theme inside frame, providing striking contrast on light page */}
      <motion.div
        initial={{ opacity: 0, y: 30 }}
        whileInView={{ opacity: 1, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.7, ease: [0.16, 1, 0.3, 1] }}
        className="relative overflow-hidden rounded-2xl border border-slate-700/60 bg-[#121316] text-[12px] leading-relaxed text-zinc-300 shadow-[0_25px_70px_-15px_rgba(0,0,0,0.35),0_0_0_1px_rgba(255,255,255,0.06)] ring-1 ring-black/40"
      >
        {/* Window Title Bar */}
        <div className="relative flex h-11 items-center justify-between border-b border-white/8 bg-[#18191e] px-4">
          {/* Traffic Lights */}
          <div className="flex items-center gap-2">
            <span className="size-3 rounded-full bg-[#ff5f56] shadow-[inset_0_0_0_1px_rgba(0,0,0,0.2)]" />
            <span className="size-3 rounded-full bg-[#ffbd2e] shadow-[inset_0_0_0_1px_rgba(0,0,0,0.2)]" />
            <span className="size-3 rounded-full bg-[#27c93f] shadow-[inset_0_0_0_1px_rgba(0,0,0,0.2)]" />
            <div className="ml-3 hidden items-center gap-2 font-mono text-[11px] text-zinc-400 sm:inline-flex">
              <span className="flex size-4.5 items-center justify-center rounded bg-lime-400/20 text-[#c4f25c]">
                <span className="ion-mark size-3" aria-hidden="true" />
              </span>
              <span className="font-bold tracking-tight text-white">ion</span>
              <span className="text-zinc-600">/</span>
              <span className="text-zinc-400">workspace</span>
            </div>
          </div>

          {/* Center Address Pill */}
          <div className="hidden items-center gap-2 rounded-full border border-white/8 bg-black/40 px-3 py-1 font-mono text-[11px] text-zinc-400 md:flex">
            <span className="size-1.5 rounded-full bg-[#c4f25c] animate-pulse" />
            <span>hierarchical-orchestrator.ts</span>
            <span className="text-zinc-600">·</span>
            <span className="text-zinc-400">Nebius Qwen-2.5-Coder-32B</span>
          </div>

          {/* Right Status Badges */}
          <div className="flex items-center gap-2">
            <button
              onClick={handleSimulate}
              disabled={isRunning}
              className="inline-flex items-center gap-1.5 rounded-md border border-white/10 bg-white/5 px-2.5 py-1 text-[11px] font-medium text-zinc-300 transition-colors hover:bg-white/10 hover:text-white"
            >
              {isRunning ? (
                <>
                  <RotateCcw className="size-3 animate-spin text-[#c4f25c]" />
                  <span>Steering...</span>
                </>
              ) : (
                <>
                  <Play className="size-3 text-[#c4f25c]" />
                  <span>Run Step</span>
                </>
              )}
            </button>
            <span className="hidden items-center gap-1 rounded bg-[#c4f25c]/10 px-2 py-0.5 font-mono text-[10px] font-medium text-[#c4f25c] sm:inline-flex">
              182 tok/s
            </span>
          </div>
        </div>

        {/* Window Body Grid: Left Sidebar + Center Workspace */}
        <div className="grid grid-cols-1 md:grid-cols-[230px_1fr]">
          {/* Left Sidebar: Hierarchical Agent Teams & Files */}
          <div className="hidden border-r border-white/6 bg-[#15161a] p-3.5 md:flex md:flex-col">
            <div className="text-[10px] font-semibold tracking-wider text-zinc-500 uppercase">
              Active Agent Team
            </div>

            <div className="mt-2.5 space-y-1">
              <div className="flex items-center justify-between rounded-lg bg-white/8 px-2.5 py-1.5 text-zinc-200">
                <div className="flex items-center gap-2">
                  <span className="size-2 rounded-full bg-[#c4f25c] animate-pulse" />
                  <span className="font-medium">Lead Architect</span>
                </div>
                <span className="text-[10px] text-zinc-500 font-mono">Lead</span>
              </div>

              <div className="pl-3 space-y-1 border-l border-white/6 ml-3">
                <div className="flex items-center justify-between rounded-md px-2 py-1 text-zinc-300 hover:bg-white/4">
                  <div className="flex items-center gap-1.5">
                    <span className="size-1.5 rounded-full bg-emerald-400" />
                    <span>Systems Coder</span>
                  </div>
                  <span className="text-[9.5px] text-zinc-500 font-mono">Pi</span>
                </div>

                <div className="flex items-center justify-between rounded-md px-2 py-1 text-zinc-300 hover:bg-white/4">
                  <div className="flex items-center gap-1.5">
                    <span className="size-1.5 rounded-full bg-cyan-400" />
                    <span>Linter & Test</span>
                  </div>
                  <span className="text-[9.5px] text-zinc-500 font-mono">OpenCode</span>
                </div>

                <div className="flex items-center justify-between rounded-md px-2 py-1 text-zinc-400 hover:bg-white/4">
                  <div className="flex items-center gap-1.5">
                    <span className="size-1.5 rounded-full bg-amber-400" />
                    <span>Reviewer Agent</span>
                  </div>
                  <span className="text-[9.5px] text-zinc-500 font-mono">Gate</span>
                </div>
              </div>
            </div>

            <div className="mt-4 text-[9.5px] font-semibold tracking-wider text-zinc-500 uppercase">
              Work & Artifacts
            </div>
            <div className="mt-1.5 space-y-0.5 font-mono text-[10.5px] text-zinc-400">
              <div className="flex items-center gap-2 rounded px-2 py-1 text-zinc-200 bg-white/5">
                <FileCode className="size-3.5 text-[#c4f25c]" />
                <span>Tasks (Active)</span>
              </div>
              <div className="flex items-center gap-2 rounded px-2 py-1 text-zinc-400 hover:text-zinc-200">
                <Workflow className="size-3.5 text-zinc-500" />
                <span>Workflows</span>
              </div>
              <div className="flex items-center gap-2 rounded px-2 py-1 text-zinc-400 hover:text-zinc-200">
                <FileText className="size-3.5 text-zinc-500" />
                <span>Artifacts</span>
              </div>
              <div className="flex items-center gap-2 rounded px-2 py-1 text-zinc-400 hover:text-zinc-200">
                <Network className="size-3.5 text-zinc-500" />
                <span>Apps (Composio)</span>
              </div>
            </div>

            <div className="mt-auto pt-4 border-t border-white/6">
              <div className="flex items-center gap-2 rounded-lg bg-black/30 p-2 text-[10.5px]">
                <Cpu className="size-3.5 text-[#c4f25c]" />
                <div>
                  <div className="font-medium text-zinc-300">Nebius Model Host</div>
                  <div className="text-[9.5px] text-zinc-500">Self-configured API</div>
                </div>
              </div>
            </div>
          </div>

          {/* Main Code & Transcript Workspace */}
          <div className="flex min-w-0 flex-col bg-[#141519]">
            {/* Tab Navigation */}
            <div className="flex items-center border-b border-white/6 bg-[#17181d] px-2 text-[11px]">
              <button
                onClick={() => setActiveTab('workflow')}
                className={`flex items-center gap-2 border-b-2 px-3 py-2.5 font-medium transition-colors ${
                  activeTab === 'workflow'
                    ? 'border-[#c4f25c] bg-[#141519] text-white'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <FileCode className="size-3.5 text-[#c4f25c]" />
                <span>autonomous_lead.ts</span>
              </button>

              <button
                onClick={() => setActiveTab('diff')}
                className={`flex items-center gap-2 border-b-2 px-3 py-2.5 font-medium transition-colors ${
                  activeTab === 'diff'
                    ? 'border-[#c4f25c] bg-[#141519] text-white'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <GitBranch className="size-3.5 text-emerald-400" />
                <span>Diff & Checkpoint</span>
                <span className="rounded bg-emerald-500/15 px-1 font-mono text-[9.5px] text-emerald-300">
                  +34 -4
                </span>
              </button>

              <button
                onClick={() => setActiveTab('artifact')}
                className={`flex items-center gap-2 border-b-2 px-3 py-2.5 font-medium transition-colors ${
                  activeTab === 'artifact'
                    ? 'border-[#c4f25c] bg-[#141519] text-white'
                    : 'border-transparent text-zinc-400 hover:text-zinc-200'
                }`}
              >
                <FileText className="size-3.5 text-cyan-400" />
                <span>Document Artifact</span>
              </button>
            </div>

            {/* Tab Content Display */}
            <div className="p-4 font-mono text-[11px] leading-relaxed md:p-5">
              <AnimatePresence mode="wait">
                {activeTab === 'workflow' && (
                  <motion.div
                    key="workflow"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.2 }}
                    className="space-y-3"
                  >
                    <div className="text-zinc-500 select-none">
                      // Ion hierarchical agent team configured with Nebius cloud model integration
                    </div>
                    <pre className="overflow-x-auto text-zinc-300">
                      <code>
{`import { AgentTeam, NebiusProvider, ComposioApps } from '@ion/core';

export const engineeringTeam = new AgentTeam({
  lead: {
    role: 'ARCHITECT',
    // Ultra-low latency open weights hosted on Nebius AI Studio
    model: NebiusProvider.model('Qwen/Qwen2.5-Coder-32B-Instruct'),
    budget: { maxTokens: 48_000, approvalGate: 'strict' }
  },
  specialists: [
    { role: 'SYSTEMS_CODER', harness: 'pi', permissions: ['fs:read', 'fs:write'] },
    { role: 'TEST_ENGINEER', harness: 'opencode', permissions: ['bash:exec'] }
  ],
  connectedApps: ComposioApps.require(['github', 'linear'])
});

// Autonomous multi-stage execution with human-in-the-loop approval
const task = await engineeringTeam.dispatch({
  objective: 'Refactor background status cache and verify zero dropped frames',
  onCheckpoint: (checkpoint) => checkpoint.requireApproval()
});`}
                      </code>
                    </pre>

                    {/* Active Step Indicator */}
                    <div className="mt-4 rounded-lg border border-white/8 bg-black/40 p-3">
                      <div className="flex items-center justify-between text-[11px]">
                        <span className="flex items-center gap-2 text-zinc-300 font-sans font-medium">
                          <Sparkles className="size-3.5 text-[#c4f25c]" />
                          Step {sessionStep} of 3: Lead reviewing specialist linter output
                        </span>
                        <span className="text-emerald-400 font-sans text-[10px]">
                          ✓ Checkpoint Gate Armed
                        </span>
                      </div>
                      <div className="mt-2 h-1.5 w-full overflow-hidden rounded-full bg-white/10">
                        <motion.div
                          className="h-full bg-[#c4f25c]"
                          animate={{ width: `${(sessionStep / 3) * 100}%` }}
                          transition={{ duration: 0.4 }}
                        />
                      </div>
                    </div>
                  </motion.div>
                )}

                {activeTab === 'diff' && (
                  <motion.div
                    key="diff"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.2 }}
                    className="space-y-3"
                  >
                    <div className="rounded-lg border border-white/8 bg-black/30 p-2.5">
                      <div className="flex items-center justify-between text-zinc-400">
                        <span>src/transcript/list.rs</span>
                        <span className="text-emerald-400 font-mono">+18 / −6</span>
                      </div>
                    </div>

                    <div className="rounded-lg border border-white/8 bg-black/50 p-3 leading-6">
                      <div className="text-rose-400/80">
                        - let status = repo.git_status(&row.path); // blocking I/O on UI thread
                      </div>
                      <div className="text-emerald-400">
                        + let status = self.status_cache.get(&row.path); // instant frame hit
                      </div>
                      <div className="text-emerald-400">
                        + cx.background_executor().spawn(refresh_cache(paths));
                      </div>
                    </div>

                    <div className="flex items-center justify-between rounded-lg border border-emerald-500/20 bg-emerald-500/10 px-3 py-2 text-emerald-300">
                      <div className="flex items-center gap-2">
                        <CircleCheck className="size-4 text-emerald-400" />
                        <span>cargo test -p ion · 94 passed, 0 failed</span>
                      </div>
                      <span className="text-xs text-emerald-400/80 font-sans">0 dropped frames</span>
                    </div>
                  </motion.div>
                )}

                {activeTab === 'artifact' && (
                  <motion.div
                    key="artifact"
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0, y: -6 }}
                    transition={{ duration: 0.2 }}
                    className="space-y-3"
                  >
                    <div className="rounded-lg border border-white/8 bg-black/40 p-3.5">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2 text-zinc-200">
                          <FileText className="size-4 text-cyan-400" />
                          <span className="font-sans font-medium">Release_Verification_Report.pdf</span>
                        </div>
                        <span className="rounded bg-white/10 px-2 py-0.5 text-[10px] text-zinc-400">
                          Ready for Export
                        </span>
                      </div>
                      <p className="mt-2 font-sans text-xs text-zinc-400 leading-normal">
                        Generated automatically by Reviewer Agent. Contains SHA-256 binary validation, frame render telemetry, and connected app audit trail for SOC2 compliance.
                      </p>
                      <div className="mt-3 flex items-center gap-2 text-[10.5px] text-[#c4f25c]">
                        <span>✓ Signed with local developer credentials</span>
                        <span className="text-zinc-600">·</span>
                        <span>42 KB</span>
                      </div>
                    </div>
                  </motion.div>
                )}
              </AnimatePresence>
            </div>
          </div>
        </div>
      </motion.div>

      {/* Floating Offset QA Inspector Window (Inspired by the top right element in the screenshot!) */}
      <motion.div
        initial={{ opacity: 0, x: 25, y: -20 }}
        whileInView={{ opacity: 1, x: 0, y: 0 }}
        viewport={{ once: true }}
        transition={{ duration: 0.8, delay: 0.2, ease: [0.16, 1, 0.3, 1] }}
        className="hidden lg:block absolute -top-8 -right-8 w-80 rounded-xl border border-slate-700/70 bg-[#16171c]/95 p-3.5 font-mono text-[11px] text-zinc-300 shadow-[0_20px_50px_rgba(0,0,0,0.4)] backdrop-blur-md ring-1 ring-white/10"
      >
        <div className="flex items-center justify-between border-b border-white/8 pb-2 text-[10px] text-zinc-400">
          <div className="flex items-center gap-1.5">
            <span className="size-2 rounded-full bg-[#c4f25c]" />
            <span className="text-zinc-300">AUTONOMOUS_QA_RUNNER</span>
          </div>
          <span className="text-zinc-500">GET /admin/auth/session</span>
        </div>
        <div className="mt-2.5 space-y-1 text-[10px] leading-5 text-zinc-400">
          <div><span className="text-purple-400">const</span> qa = <span className="text-blue-400">new</span> Agent(&#123; role: <span className="text-emerald-300">'QA_ENGINEER'</span> &#125;);</div>
          <div><span className="text-blue-400">await</span> qa.runSession(<span className="text-blue-400">async</span> (browser) =&gt; &#123;</div>
          <div className="pl-3 text-zinc-500">// Agent navigates to local preview</div>
          <div className="pl-3"><span className="text-blue-400">await</span> browser.goto(<span className="text-emerald-300">'http://localhost:3000'</span>);</div>
          <div className="pl-3"><span className="text-blue-400">await</span> browser.click(<span className="text-emerald-300">'[data-testid="deploy-btn"]'</span>);</div>
          <div className="pl-3 text-zinc-500">// Visual inspection & remediation</div>
          <div className="pl-3"><span className="text-blue-400">const</span> ok = <span className="text-blue-400">await</span> browser.isVisible(<span className="text-emerald-300">'.toast'</span>);</div>
          <div className="pl-3"><span className="text-purple-400">if</span> (!ok) <span className="text-blue-400">await</span> qa.fix(<span className="text-emerald-300">'deploy_toast_missing'</span>);</div>
          <div>&#125;);</div>
        </div>
        <div className="mt-2.5 flex items-center justify-between border-t border-white/6 pt-2 text-[9.5px]">
          <span className="text-emerald-400">● 100% test assertions verified</span>
          <span className="text-zinc-500">Autonomous QA</span>
        </div>
      </motion.div>
    </div>
  )
}
