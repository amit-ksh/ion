import type {
  FileEntry,
  ComposerApp,
  Team,
  ProviderKind,
  ProviderModelOption,
  ReportedCommand,
  SlashCommand,
} from './generated'
import { fuzzyScore } from './fuzzy-search'

export const COMPOSER_AUTOCOMPLETE_CAP = 64

export type ComposerTriggerKind = 'command' | 'file' | 'team' | 'app'

export interface ComposerTrigger {
  kind: ComposerTriggerKind
  query: string
  start: number
  end: number
}

export type ComposerAutocompleteRow =
  | { kind: 'command'; command: SlashCommand }
  | { kind: 'file'; file: FileEntry }
  | { kind: 'app'; app: ComposerApp }
  | { kind: 'team'; team: Team; mention: string; invoke: boolean }

export function detectComposerTrigger(text: string, cursor: number): ComposerTrigger | null {
  const end = Math.max(0, Math.min(cursor, text.length))
  const lineStart = text.lastIndexOf('\n', end - 1) + 1
  const linePrefix = text.slice(lineStart, end)
  if (linePrefix.startsWith('/')) {
    const query = linePrefix.slice(1)
    if (!/\s/u.test(query))
      return { kind: 'command', query, start: lineStart, end }
  }

  const prefix = text.slice(0, end)
  const braced = prefix.match(/(?:^|\s)(@@\{[^}\r\n]*)$/u)
  if (braced)
    return {
      kind: 'team',
      query: braced[1]!.slice(2),
      start: end - braced[1]!.length,
      end,
    }
  const legacyBraced = prefix.match(/^\s*(@(?:team:)?\{[^}\r\n]*)$/u)
  if (legacyBraced)
    return {
      kind: 'file',
      query: legacyBraced[1]!.slice(1),
      start: end - legacyBraced[1]!.length,
      end,
    }
  let tokenStart = end
  while (tokenStart > 0 && !/\s/u.test(text[tokenStart - 1]!)) tokenStart -= 1
  const token = text.slice(tokenStart, end)
  if (!token.startsWith('@') || token.startsWith('@@@@')) return null
  const length = token.startsWith('@@@') ? 3 : token.startsWith('@@') ? 2 : 1
  return { kind: length === 3 ? 'app' : length === 2 ? 'team' : 'file', query: token.slice(length), start: tokenStart, end,
  }
}

export function mergeComposerCommands(
  discovered: SlashCommand[],
  reported: ReportedCommand[],
): SlashCommand[] {
  const merged = discovered.map((command) => ({ ...command }))
  for (const report of reported) {
    const known = merged.find((command) => command.name === report.name)
    if (known) {
      if (!known.description) known.description = report.description ?? ''
      continue
    }
    merged.push({
      name: report.name,
      description: report.description ?? '',
      scope: 'Builtin',
      argument_hint: null,
      template: null,
    })
  }
  return merged.sort((left, right) => {
    const byScope = commandScopeRank(left.scope) - commandScopeRank(right.scope)
    return byScope || left.name.localeCompare(right.name)
  })
}

export function isFastModeToggleSubmission(
  provider: ProviderKind,
  prompt: string,
  commands: SlashCommand[],
): boolean {
  return (
    provider === 'codex'
    && prompt.trim() === '/fast'
    && commands.some((command) => command.name === 'fast'
      && command.scope === 'Builtin'
      && command.template === null,
    )
  )
}

/** Waku's provider-neutral terminal-session picker. */
export function isResumeSubmission(prompt: string): boolean {
  return prompt.trim() === '/resume'
}

export type GoalCommand =
  | { kind: 'show' }
  | { kind: 'edit' }
  | { kind: 'pause' }
  | { kind: 'resume' }
  | { kind: 'clear' }
  | { kind: 'set'; objective: string }

/**
 * Parse the submitted text as Codex's native `/goal` command, which Waku
 * bridges to `thread/goal/*`. `null` when it is not one — wrong provider,
 * other text, or a project/user command that deliberately owns `/goal`
 * (resolution precedence stands).
 */
export function parseGoalSubmission(
  provider: ProviderKind,
  prompt: string,
  commands: SlashCommand[],
): GoalCommand | null {
  if (provider !== 'codex') return null
  const invocation = prompt.trim()
  if (!invocation.startsWith('/')) return null
  const body = invocation.slice(1)
  const split = body.match(/^(\S+)(?:\s+([\s\S]*))?$/u)
  if (!split || split[1] !== 'goal') return null
  const goalIsCodexBuiltin = commands.some((command) => command.name === 'goal'
    && command.scope === 'Builtin'
    && command.template === null,
  )
  if (!goalIsCodexBuiltin) return null
  const argument = (split[2] ?? '').trim()
  switch (argument) {
    case '': return { kind: 'show' }
    case 'edit': return { kind: 'edit' }
    case 'pause': return { kind: 'pause' }
    case 'resume': return { kind: 'resume' }
    case 'clear': return { kind: 'clear' }
    default: return { kind: 'set', objective: argument }
  }
}

export function toggledFastServiceTier(
  current: string | null | undefined,
  serviceTiers: ProviderModelOption[],
): string | null {
  const fast = serviceTiers.find((tier) => ['fast', 'priority'].includes(tier.id)
    || tier.label.toLocaleLowerCase() === 'fast',
  )
  if (!fast) return null
  return current === fast.id ? 'default' : fast.id
}

export interface ComposerReferences {
  apps: ComposerApp[]
  teams: Team[]
  allowTeams: boolean
  invokeTeams?: boolean
}

export function composerAutocompleteRows(
  trigger: ComposerTrigger,
  commands: SlashCommand[],
  files: FileEntry[],
  cap = COMPOSER_AUTOCOMPLETE_CAP,
  references: ComposerReferences = { apps: [], teams: [], allowTeams: false },
): ComposerAutocompleteRow[] {
  const rank = <T>(items: T[], candidate: (item: T) => string, query: string, limit: number): T[] => {
    if (!query.trim()) return items.slice(0, limit)
    return items.map((item, index) => ({ item,
      index,
      score: fuzzyScore(query, candidate(item)) }))
    .filter((item): item is typeof item & { score: number } => item.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .slice(0, limit).map(({ item }) => item)
  }
  if (trigger.kind === 'command')
    return rank(commands, (item) => item.name, trigger.query, cap)
    .map((command) => ({
      kind: 'command',
      command,
    }))
  const query = trigger.query.replace(/^(app:|team:)/u, '').replace(/^[{]/u, '')
  const referenceCap = trigger.kind === 'file' ? Math.min(16, Math.floor(cap / 3)) : cap
  const teams: ComposerAutocompleteRow[] =
    !references.allowTeams || trigger.kind === 'app'
      ? []
      : rank(references.teams, (item) => item.name, query, referenceCap).map((team) => ({
          kind: 'team',
          team,
          mention: teamReference(team, references.teams),
          invoke: references.invokeTeams ?? true,
        }))
  const apps: ComposerAutocompleteRow[] =
    trigger.kind === 'team'
      ? []
      : rank(references.apps, (item) => `${item.name} ${item.slug}`, query, referenceCap).map(
          (app) => ({ kind: 'app', app }),
        )
  if (trigger.kind === 'team') return teams
  if (trigger.kind === 'app') return apps
  // Keep all three categories reachable even when a checkout has 50,000 files.
  const fileRows: ComposerAutocompleteRow[] = rank(
    files,
    (item) => item.path,
    trigger.query,
    cap - teams.length - apps.length,
  ).map((file) => ({ kind: 'file', file }))
  return [...fileRows, ...teams, ...apps]
}

export function replaceComposerTrigger(
  text: string,
  trigger: ComposerTrigger,
  row: ComposerAutocompleteRow,
): { text: string; cursor: number } {
  const insert = row.kind === 'command'
    ? `/${row.command.name} `
    : row.kind === 'team'
        ? `${row.mention} `
        : row.kind === 'app'
          ? `@@@${row.app.slug} `
          : `@${row.file.path.startsWith('@') ? './' : ''}${row.file.path} `
  const next = `${text.slice(0, trigger.start)}${insert}${text.slice(trigger.end)}`
  return { text: next, cursor: trigger.start + insert.length }
}

export function expandCommandTemplate(template: string, args: string): string {
  const positional = args.split(/\s+/u).filter(Boolean)
  let expanded = ''
  let consumedArgs = false
  let rest = template
  while (true) {
    const index = rest.indexOf('$')
    if (index < 0) break
    expanded += rest.slice(0, index)
    const after = rest.slice(index + 1)
    if (after.startsWith('ARGUMENTS')) {
      expanded += args
      consumedArgs = true
      rest = after.slice('ARGUMENTS'.length)
    } else if (after.startsWith('@')) {
      expanded += args
      consumedArgs = true
      rest = after.slice(1)
    } else if (/^[1-9]/u.test(after)) {
      expanded += positional[Number(after[0]) - 1] ?? ''
      consumedArgs = true
      rest = after.slice(1)
    } else {
      expanded += '$'
      rest = after
    }
  }
  expanded += rest
  if (!consumedArgs && args) expanded += `\n\n${args}`
  return expanded
}

export function expandedComposerSubmission(
  provider: ProviderKind,
  prompt: string,
  commands: SlashCommand[],
): string | null {
  if (!prompt.startsWith('/')) return null
  const invocation = prompt.slice(1)
  const whitespace = invocation.search(/\s/u)
  const name = whitespace < 0 ? invocation : invocation.slice(0, whitespace)
  const args = whitespace < 0 ? '' : invocation.slice(whitespace).trim()
  const skill = commands.find((item) => item.name === name && item.scope === 'Skill')
  if (skill) {
    if (provider === 'codex' || provider === 'fx') return `$${invocation}`
    if (provider === 'pi' || provider === 'ohMyPi') return `/skill:${invocation}`
  }
  const command = commands.find((item) => item.name === name && item.template !== null)
  return command?.template === null || command?.template === undefined
    ? null
    : expandCommandTemplate(command.template, args)
}

function commandScopeRank(scope: SlashCommand['scope']): number {
  switch (scope) {
    case 'Builtin':
    case 'Waku':
      return 0
    case 'Project':
      return 1
    case 'User':
      return 2
    case 'Skill':
      return 3
  }
}

export function teamReference(team: Team, teams: Team[]): string {
  const name = team.name.trim()
  return /[{}\n\r]/u.test(name) ||
    teams.filter((item) => item.name.trim().toLowerCase() === name.toLowerCase()).length !== 1
    ? `@@${team.id}`
    : `@@{${name}}`
}

/** The daemon re-resolves the recipient and pins access/budgets on durable queueing. */
export async function queueComposerTeam(
  client: import('./client').WakuClient,
  session: import('./generated').AgentSession,
  prompt: string,
  attachments: import('./generated').MessageAttachment[],
  validate?: (team: Team, snapshot: import('./generated').ControlSnapshot) => void,
): Promise<import('./generated').AgentSession | null> {
  if (!/^\s*(@@(?!@)|#team:|@team:)/u.test(prompt)) return null
  if (
    session.turns.length ||
    session.messages.length ||
    session.provider_cursor ||
    session.last_reply_at ||
    ['connecting', 'working', 'waiting', 'background'].includes(session.status)
  ) {
    if (/^\s*(@@(?!@)|#team:)/u.test(prompt)) return null
    throw new Error('Use New task to start a team task.')
  }
  const match = prompt
    .trimStart()
    .match(/^(?:@@(?!@)|[#@]team:)(?:\{([^}\r\n]+)\}|([^\s{}]+))\s+([\s\S]+)$/u)
  if (!match || !match[3]!.trim()) throw new Error('Choose a team with @@, then describe the task.')
  const response = await client.request({ type: 'loadWorkspaceControl' })
  if (response.type !== 'workspaceControl') throw new Error('Could not load teams. Try again.')
  const teams = response.snapshot.records.filter(
    (item): item is Team & { kind: 'team' } => item.kind === 'team',
  )
  const name = match[1] ?? match[2]!
  const isId = /^[0-9a-f]{8}-(?:[0-9a-f]{4}-){3}[0-9a-f]{12}$/iu.test(name)
  const matches = teams.filter((item) =>
    isId
      ? item.id.toLowerCase() === name.toLowerCase()
      : item.name.trim().toLowerCase() === name.toLowerCase(),
  )
  if (matches.length !== 1)
    throw new Error('This team is unavailable or ambiguous. Choose it from @@ suggestions.')
  validate?.(matches[0]!, response.snapshot)
  const queued = await client.request(
    {
      type: 'queueTeamTask',
      runId: session.id,
      projectId: session.project_id,
      expectedDefinitionRevision: response.snapshot.revision,
      teamId: matches[0]!.id,
      sourcePrompt: prompt,
      attachments,
    },
    session.id,
  )
  if (queued.type !== 'teamTaskQueued' || queued.session.id !== session.id)
    throw new Error('Team acknowledgement missing. Refresh Tasks before retrying.')
  return queued.session
}
