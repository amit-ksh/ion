# Generated PDF artifacts

Updated 2026-10-11. Development branch: `codex/ion-agent-teams`.

## Behavior

The shared native Teams tools include `save_pdf`. It takes a `.pdf` filename,
canonical standard base64 containing up to 24 KiB of generated PDF bytes, and an
agent-authored summary up to 4 KiB. It does not import a user-selected file.
The existing assignment lease, project, task, agent and request identity determine
provenance. Payload, artifact record, immutable summary and receipt commit together.
Exact retries return the same artifact. A changed name, content or summary fails.

The owner verifies the file's PDF header and final EOF marker, size and encoding.
These are envelope checks. They do not prove that a PDF renders or that its contents
are accurate. PDF parsing, OCR and embedded previews remain separate features.
The file is treated as opaque bytes, including PDFs that are not valid UTF-8.

Artifacts export uses the existing exact-byte download path. Hashes and original
producer metadata are verified before export, report acceptance, child review,
source-task reads and workflow handoff. The summary is sealed together with the
artifact metadata; a missing or modified summary prevents use rather than being
silently repaired. Clients cannot add or replace these generated artifacts.

`read_document` and normal tasks' `read_team_document` return a labeled agent
summary, byte count and original hash for a generated PDF. They do not return
extracted PDF text or send PDF bytes into model context. Text-document reads keep
their existing excerpts. The normal task can only read a completed team invoked
from that exact source task and project.

Workflow stages can require PDF output in the existing document-format control.
The setting remains pinned and visible on desktop and phone. A real accepted PDF
artifact is required for completion. A subsequent stage receives an immutable
JSON artifact descriptor and the labeled summary, while the original PDF remains
in Artifacts. The descriptor is named `<artifact-id>.artifact.json`; a summary is
never presented as a `.pdf` file or interpreted content.

An interrupted native `save_pdf` acknowledgment is matched to its exact persisted
arguments and receipt during existing explicit recovery. Recovery acknowledges
that original artifact without repeating publication. Unknown/uncommitted calls
remain unexecuted, and changed pending arguments prevent continuation.

All operations run on the existing owner/provider workers. No filesystem walks,
PDF process, parser or network work is added to UI rendering. Existing selected
harnesses share the tool path; no new harness adapter is introduced.

## Review checklist

- [ ] Ask a saved team for a small PDF report and an accurate accompanying summary.
- [ ] Open Artifacts, export the report, and inspect it in your PDF viewer.
- [ ] Confirm task, project, producer, byte count and hash identify the real run.
- [ ] Ask the team lead to inspect a child's PDF. Confirm its text is explicitly
  identified as the agent's summary and no extraction/OCR is claimed.
- [ ] Invoke a team from a normal task and read its PDF result after completion.
- [ ] Require PDF in the first stage of a two-stage workflow. Confirm missing PDF
  prevents completion and the second stage receives the correct JSON descriptor.
- [ ] In a disposable fixture, change the receipt bytes, name or summary; report,
  review, reads and export must reject the changed evidence.
- [ ] Interrupt the publication acknowledgment and use explicit recovery. Confirm
  there is still one PDF artifact and the original file bytes are retained.
- [ ] Try invalid base64, a missing PDF envelope, a larger file or a stale lease;
  publication must fail without adding an artifact.

## Verification

Nine new PDF/native/workflow/recovery/source/excerpt regressions, three existing
text-document tests, three artifact-integrity tests, two workflow document tests
and the existing exact source-read scope regression passed. Desktop, daemon and
native worker compilation, protocol 37 binding equality, formatting and
whitespace checks passed. Two fixture mistakes (Stop revision and occupied saved
agent capacity) were corrected without weakening production controls.
No visual test, live provider call, account mutation, remote deployment or billable
provisioning is included. Larger binary transfers and additional binary formats
remain development work.
