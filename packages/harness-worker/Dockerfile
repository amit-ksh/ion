# Build from an exported, reviewed source snapshot, never the whole checkout.
# Both bases must be supplied as reviewed sha256-pinned Linux images.
ARG BUILD_BASE
ARG RUNTIME_BASE
FROM ${BUILD_BASE} AS build
ARG SOURCE_REVISION
WORKDIR /ion-build
COPY source.tar /tmp/ion-source.tar
RUN test -n "$SOURCE_REVISION" && tar -xf /tmp/ion-source.tar -C /ion-build \
    && rm /tmp/ion-source.tar
ENV CARGO_INCREMENTAL=0 CARGO_TARGET_DIR=/ion-release-target
# Preserve the repository release profile (thin LTO, one codegen unit, stripped).
# A cached reviewed builder can have the native dependencies vendored already.
ARG CARGO_OFFLINE=false
RUN if [ "$CARGO_OFFLINE" = true ]; then export CARGO_NET_OFFLINE=true; fi; \
    cargo build --release --locked --jobs 2 -p waku-core --bin ion-harness-worker
RUN install -Dm0555 /ion-release-target/release/ion-harness-worker /ion-build/worker

FROM ${RUNTIME_BASE} AS runtime
ARG SOURCE_REVISION
LABEL org.opencontainers.image.title="Ion harness worker" \
      org.opencontainers.image.revision="$SOURCE_REVISION"
USER 0:0
# Refuse a conflicting existing identity rather than inheriting another user's home.
RUN ! getent passwd 1000 && ! getent group 1000 \
    && groupadd --gid 1000 ion \
    && useradd --uid 1000 --gid 1000 --home-dir /home/ion --create-home ion \
    && install -d -m 0755 /ion /ion/bootstrap \
    && install -d -m 0700 -o ion -g ion /home/ion/.config /ion/work \
    && find / -xdev -type f -perm /6000 -exec chmod a-s {} +
COPY --from=build --chmod=0555 /ion-build/worker /usr/local/bin/ion-harness-worker
# The ABI check must run without a provider login or request.
RUN /usr/local/bin/ion-harness-worker --invalid-contract >/tmp/ion-worker.out 2>/tmp/ion-worker.err; \
    result=$?; test "$result" = 2 && test ! -s /tmp/ion-worker.out \
    && grep -q 'no arguments' /tmp/ion-worker.err \
    && rm /tmp/ion-worker.out /tmp/ion-worker.err
ENV HOME=/home/ion
USER 1000:1000
WORKDIR /ion/work
ENTRYPOINT ["/usr/local/bin/ion-harness-worker"]
