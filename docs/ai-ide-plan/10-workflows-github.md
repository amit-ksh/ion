# Milestone 3: specialist workflow and GitHub path

The native Nebius lead can delegate one sequential researcher or reviewer
objective at a time. Each specialist receives only its objective and the
read-only project file tools. The daemon journals a typed child task, its
status, model/tool messages, and a text artifact before returning the artifact
to the lead. A restart marks an unfinished child as interrupted; it does not
silently rerun it. The user can ask the lead to repeat a completed specialist
task with the same contract; the new child records a dependency on the prior
child.

The lead can read a GitHub issue through the user's local `gh` CLI. The tool
result includes the issue URL and bounded body as source attribution. A draft
pull request can target only the workspace's `origin` repository and current
local branch. The proposed repository, branches, their commit SHAs, title, and
complete body are shown in a one-time approval. ion checks the branch tips
again after approval and does not push commits. The CLI retains its
own GitHub credentials; no token is copied into the task journal or model
context. The later personal gateway will replace this local bridge for remote
connected-account actions.

An approved PR request is journaled before the external call. If the process
stops during submission, the proposal becomes **uncertain** and ion will not
retry it automatically. Review GitHub manually before attempting another PR
for that branch pair.

## Setup

- Keep the existing `bun ./scripts/dev.ts` watcher running; start it only if
  absent. Wait for app and daemon rebuilds after code changes. On Windows the
  watcher briefly stops and relaunches the app for a daemon rebuild because
  Cargo cannot replace a running `.exe`.
- Save and test a Nebius Token Factory key in **Settings → Models & Speech**.
  The live Milestone 2 key check remains pending in this environment.
- Install GitHub CLI (`gh`) and complete `gh auth login` on this computer for
  the repository you intend to use. Keep its authentication outside ion.
- For PR creation, use a disposable repository or branch, push the current
  branch yourself, and know the intended base branch.

## Milestone 3 review checklist

- [ ] Create a Nebius task in a small local project. Ask the lead to delegate
      a file review to a `reviewer` specialist. Confirm one child task activity
      appears and the lead receives a report artifact with file evidence.
- [ ] Confirm the specialist can list/read files but cannot edit, delegate,
      or use GitHub. Interrupt a child run and reopen the task; confirm it is
      marked interrupted and is not replayed automatically.
- [ ] Ask the lead to repeat a completed specialist task. Confirm a new child
      and artifact appear with the same objective and read-only grant.
- [ ] Ask the lead to read a GitHub issue. Confirm its source URL and title
      appear, and the issue body is attributed to that URL.
- [ ] From a pushed branch in the workspace's GitHub repository, ask the lead
      to propose a draft PR. Review the exact repository, head/base branches,
      commit SHAs, title, and body in the approval card. First choose **Deny** and confirm
      no PR was created.
- [ ] Make a second proposal and approve it. Confirm the resulting draft PR
      URL, repository, branches, title, and body on GitHub. Check that no
      duplicate PR appears after reopening or retrying the task.
- [ ] Confirm a proposal targeting another repository or a different local
      branch is rejected before approval. Confirm a missing `gh` login gives
      an actionable setup error without exposing a token.
- [ ] Review the real-provider child event and GitHub result, then explicitly
      approve Milestone 3 or request fixes.

The Milestone 2 live check and this Milestone 3 live check require the user's
own Nebius key. No real provider payload has been verified in this environment
yet.

The local bridge uses the [GitHub CLI authenticated API](https://cli.github.com/manual/gh_api)
and [GitHub pull request API](https://docs.github.com/en/rest/pulls/pulls#create-a-pull-request).
