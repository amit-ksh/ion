# NVIDIA and Nebius runtime evidence

The [README](README.md) is the canonical decision record. This note retains
the primary sources behind the Nemotron and Nebius choices.

- [Token Factory API documentation](https://docs.tokenfactory.nebius.com/)
  defines the fixed HTTPS inference endpoint and authenticated model catalog.
  Discover the user's available model IDs at runtime; do not infer API IDs
  from product labels.
- [Nemotron 3 Super cookbook](https://github.com/nebius/token-factory-cookbook/blob/main/models/nemotron/nemotron3-super-120B.md)
  supplies Token Factory examples for NVIDIA model inference. The native
  runtime must verify actual streaming and tool payload order.
- [Nebius Serverless AI](https://docs.nebius.com/serverless) describes Jobs
  for finite work and Endpoints for served workloads. A user-owned sandbox or
  Job may later execute explicitly delegated cloud work; the daemon still
  owns durable task state.
- [NeMo-Speech.cpp ASR models](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/asr/models.md)
  lists Nemotron 3.5 ASR Streaming 0.6B as the local multilingual model.
  Its [CLI](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/cli.md)
  and [native SDK](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/sdk.md)
  document streaming integration.
- [NemoClaw](https://github.com/NVIDIA/NemoClaw) is evidence for later
  sandbox and credential policy work. ion may add it only as an optional,
  feature-flagged adapter after compatibility checks. It never owns team
  state. Eve is excluded.

The approved hackathon flow uses native Nemotron execution first. Agent
teams, cloud execution, local speech, and a personal gateway follow only
through their milestone approval gates.
