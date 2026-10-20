# Managed runtime contract notes — 19 October 2026

Primary sources consulted for the implementation:

- [NeMo-Speech.cpp HTTP/WebSocket reference](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/api.md): the existing Ion clients use this transcription-specific binary PCM16/event protocol. It differs from a generic OpenAI realtime session and from the NIM protocol. The managed broker keeps the existing contract and gates access.
- [NeMo-Speech.cpp server guidance](https://github.com/NVIDIA/NeMo-Speech.cpp/blob/main/docs/server.md): the existing operator setup remains the pilot backend; source-level integration does not establish production capacity or model quality.
- [Docker run reference](https://docs.docker.com/reference/cli/docker/container/run/): container memory/CPU/process/read-only/security options and driver-dependent storage limits are configuration, not proof that an arbitrary host enforces them. The worker requires the selected quota-capable host profile.
- [gVisor Docker setup](https://gvisor.dev/docs/user_guide/quick_start/docker/): a registered runsc runtime must be installed/configured before using the worker.
- [Nebius S3 compatibility](https://docs.nebius.com/object-storage/interfaces/s3-api-compatibility): bucket static website hosting is unsupported. The pilot therefore serves immutable objects through Ion's separate content origin and private disk.

Implementation decision: deploy the pilot control API, trusted NeMo worker and a separate isolated Linux build/runtime worker on operator-managed infrastructure. No Nebius resource creation API, purchased resource, image pull or model download was executed here. The VM/region, TLS/DNS, exact images, quota support and firewall must be accepted on the actual target.

The speech admission cap of four and worker caps in implementation 111 are protective policy defaults. They are not benchmark results. Runtime/device testing is deferred by the owner.
