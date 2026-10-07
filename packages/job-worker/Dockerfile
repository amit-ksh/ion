# Supply a reviewed Python Linux base pinned by sha256 digest.
ARG PYTHON_BASE
FROM ${PYTHON_BASE}
RUN groupadd --gid 1000 ion && useradd --uid 1000 --gid 1000 --create-home ion \
    && mkdir -p /ion && chown root:root /ion
COPY --chmod=0555 ion_job_worker.py /usr/local/bin/ion-job-worker
COPY --chmod=0444 job-host.json /ion/job-host.json
# Remove native privilege elevation from the reviewed image.
RUN find / -xdev -type f -perm /6000 -exec chmod a-s {} +
# Only the fixed supervisor starts as root; Popen demotes command groups/UID/GID.
USER 0:0
WORKDIR /ion
ENTRYPOINT ["/usr/local/bin/ion-job-worker"]
