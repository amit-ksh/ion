# Milestone 6: optional NemoClaw execution adapter

The user authorized this milestone while the earlier live review checklists
remain open. The native Nebius runtime and ion daemon still own the task,
approvals, journal, and team state. NemoClaw supplies only a user-managed
OpenShell sandbox for an explicitly approved one-off command. The adapter is
disabled by default and does not provision, rebuild, or alter sandbox policy.

## Development checklist

- [x] Hide the `nemoclaw_exec` model tool unless `ION_NEMOCLAW_ADAPTER=1` and a
      valid `ION_NEMOCLAW_SANDBOX` name are configured. Reject malformed names.
- [x] Validate a bounded argument array and pass it literally to
      `nemoclaw <sandbox> exec --no-tty -- <command...>`. Windows uses WSL 2;
      `ION_NEMOCLAW_WSL_DISTRO` may select a distribution, otherwise WSL's
      default distribution is used. No host shell interpolates model text.
- [x] Check that the named sandbox can run a harmless `true` command before
      showing an approval. Show the exact argument array, sandbox name, state
      change warning, and disclosure that stdout returns to the Nebius model.
- [x] Record a pending effect before dispatch. Cap command time at two minutes
      and visible stdout at 16 KiB. Keep CLI stderr out of model context. On
      cancellation, timeout, or process interruption, report an uncertain
      outcome and never replay the command during journal recovery.
- [x] Keep the specialist tool grant read-only; specialists cannot invoke the
      NemoClaw adapter. The local workspace is not mounted or copied into the
      sandbox by ion.
- [ ] Validate the adapter against a real user-managed NemoClaw sandbox and a
      live Nebius model after the owner completes the setup below.

## Owner setup for live review

1. Install and onboard NemoClaw in a disposable sandbox using NVIDIA's
   [quickstart](https://docs.nvidia.com/nemoclaw/latest/get-started/quickstart.html).
   On Windows, first follow NVIDIA's
   [WSL 2 preparation](https://docs.nvidia.com/nemoclaw/latest/get-started/windows-setup.html).
   The sandbox, container runtime, inference account, and their charges remain
   yours. Do not paste credentials into ion's task or this checklist.
2. From a terminal in the same WSL distribution, run `nemoclaw list`, then
   `nemoclaw <sandbox> exec --no-tty -- true`. This confirms the CLI and the
   exact sandbox name. NVIDIA documents `nemoclaw <name> exec` as the managed
   [one-off sandbox command](https://docs.nvidia.com/nemoclaw/latest/user-guide/openclaw/reference/commands).
3. Set `ION_NEMOCLAW_ADAPTER=1` and `ION_NEMOCLAW_SANDBOX=<sandbox>` in the
   environment that starts ion's desktop and daemon. On Windows, optionally
   set `ION_NEMOCLAW_WSL_DISTRO=<distribution>` when NemoClaw is not installed
   in the default WSL distribution. Restart that process after setting them.
   Leave the flag unset to disable the adapter. The running development
   watcher owns its app process; change its environment only when you are
   ready to restart it for review.
4. Configure and test your Nebius Token Factory key in **Settings → Models &
   Speech**. A live native Nemotron task needs that key before it can propose
   the sandbox tool. NemoClaw's own inference route and credentials are managed
   by NemoClaw, not copied from ion.

## Review checklist

- [ ] With the flag unset, start a native Nemotron task and confirm the
      NemoClaw tool is unavailable while normal local tools still work.
- [ ] Enable the flag with the disposable sandbox. Ask for a harmless command
      such as `pwd`. Confirm the approval shows the exact sandbox and argument
      array. Deny it once; confirm nothing ran. Repeat and allow once; confirm
      stdout is bounded and recorded in the task's ordered activity.
- [ ] Run an argument containing spaces or shell metacharacters as one literal
      argument; confirm the host does not interpret it. Confirm the command
      runs in the sandbox user context, not the local ion workspace.
- [ ] Try an unavailable sandbox and a stopped sandbox. Confirm ion reports a
      setup error before approval and leaves the native local task usable.
- [ ] Stop ion while an approved sandbox command is running, then reopen the
      task. Inspect the sandbox result yourself. Confirm the journal reports an
      uncertain outcome and does not execute the command a second time.
- [ ] Check the sandbox's OpenShell policy and inference route with NemoClaw's
      own status tools. Confirm no ion Nebius key, desktop gateway pairing
      token, or local project is present in the sandbox unless you explicitly
      provided it there. Do not run `env` or secret-reading commands during a
      model task: approved stdout enters the model context.
- [ ] Review the real run and explicitly approve Milestone 6 or request fixes.

No live sandbox or Nebius key was available during implementation. Static
compilation and local recovery tests verify the ion boundary; they do not
prove a specific NemoClaw installation, OpenShell policy, or provider route.
