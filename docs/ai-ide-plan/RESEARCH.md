# Research inventory

The [README](README.md) is the canonical product decision record and
[developer resources](08-byok-nebius-development-resources.md) contains the
implementation links. This inventory records the evidence still relevant to
the approved free/BYOK plan. It makes no separate product decisions.

- The [Nebius x NVIDIA hackathon](https://nebiusglobalaihackathon.devpost.com/)
  requires substantive Nebius and NVIDIA use. Check its current rules and
  deadline directly before submission.
- [Token Factory documentation](https://docs.tokenfactory.nebius.com/)
  covers API keys, inference, model catalog, and account limits. ion uses the
  user's own key at the fixed Token Factory endpoint.
- [Nebius Serverless AI](https://docs.nebius.com/serverless) distinguishes
  finite Jobs from served Endpoints. Neither replaces the local daemon's
  task database.
- [Composio documentation](https://docs.composio.dev/) covers connected
  accounts and scoped tool sessions. The later personal gateway is the
  credential and audit boundary.
- [Waku daemon security](../../crates/waku-daemon/README.md) documents the
  existing full-control daemon token. A later mobile path requires scoped
  pairing and encrypted transport.
- [NVIDIA NeMo-Speech.cpp](https://github.com/NVIDIA/NeMo-Speech.cpp) provides
  the local runtime for Nemotron 3.5 ASR Streaming 0.6B.

Implementation should re-verify live API behavior against these primary
sources and actual provider payloads. See the chapter links in the README
for architecture, milestones, and approval gates.
