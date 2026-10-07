# Supply a reviewed Python Linux base pinned by sha256 digest.
ARG PYTHON_BASE
FROM ${PYTHON_BASE}
# Exact signed Debian package version matching the reviewed base; no floating dependency.
ARG LIBSECCOMP_VERSION
RUN test -n "$LIBSECCOMP_VERSION" && apt-get update \
    && apt-get install -y --no-install-recommends "libseccomp2=$LIBSECCOMP_VERSION" \
    && rm -rf /var/lib/apt/lists/*
RUN groupadd --gid 1000 ion && useradd --uid 1000 --gid 1000 --create-home ion \
    && mkdir -p /ion /usr/local/libexec && chown root:root /ion /usr/local/libexec
COPY --chmod=0555 ion_job_worker.py /usr/local/bin/ion-job-worker
COPY --chmod=0555 ion_job_guard.py /usr/local/libexec/ion-job-guard.py
# Reaccept exact host kernel releases before rebuilding the immutable image.
ARG ACCEPTED_KERNEL_RELEASES
RUN python3 -I -S -c 'import json,os,re; v=os.environ.get("ACCEPTED_KERNEL_RELEASES", "").split(","); assert 1<=len(v)<=8 and len(set(v))==len(v) and all(re.fullmatch(r"[A-Za-z0-9_.+\-]{1,128}",x) for x in v); open("/usr/local/libexec/ion-job-kernels.json","w").write(json.dumps({"version":1,"releases":v},separators=(",",":")))' \
    && chmod 0444 /usr/local/libexec/ion-job-kernels.json
COPY --chmod=0444 job-host.json /ion/job-host.json
# Remove native privilege elevation from the reviewed image.
RUN find / -xdev -type f -perm /6000 -exec chmod a-s {} +
# Only the fixed supervisor starts as root; Popen demotes command groups/UID/GID.
USER 0:0
WORKDIR /ion
ENTRYPOINT ["/usr/local/bin/ion-job-worker"]
