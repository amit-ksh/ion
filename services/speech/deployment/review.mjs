import { randomUUID } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { fail, canceled, stableFile, json, fields } from "./files.mjs";
export const DEVICES = [
  { id: "iphone15", model: "iPhone 15", platform: "ios" },
  { id: "moto-g32", model: "Moto G32", platform: "android" },
];
export const CASES = [
  {
    id: "install",
    requires_origin: false,
    facts: ["native_build_installed", "app_opens", "correct_build_verified"],
  },
  {
    id: "readiness_no_audio",
    requires_origin: true,
    facts: ["test_passes", "no_mic_permission_prompt", "no_audio_sent"],
  },
  {
    id: "incorrect_token",
    requires_origin: true,
    facts: ["test_refuses", "no_audio_sent", "no_token_in_error"],
  },
  {
    id: "saved_origin_reuse",
    requires_origin: true,
    facts: ["explicit_test_passes", "token_not_reentered"],
  },
  {
    id: "changed_origin",
    requires_origin: true,
    facts: ["new_origin_requires_new_token", "old_token_not_forwarded"],
  },
  {
    id: "permission_denied",
    requires_origin: false,
    facts: ["recording_refuses", "mic_indicator_off", "draft_unchanged"],
  },
  ...[1, 10, 30, 60].map((seconds) => ({
    id: `dictation_${seconds}s`,
    requires_origin: true,
    facts: [
      "partial_preview",
      "all_expected_final_words",
      "stop_acknowledged",
      "text_editable",
      "task_not_auto_submitted",
    ],
  })),
  {
    id: "hindi_english",
    requires_origin: true,
    facts: [
      "intended_languages_recognized",
      "expected_words_reviewed",
      "final_text_editable",
    ],
  },
  ...[
    "cancel_preparation",
    "cancel_finalization",
    "task_change",
    "daemon_change",
    "background",
    "headset_disconnect",
  ].map((id) => ({
    id,
    requires_origin: true,
    facts: [
      "capture_stopped",
      "mic_indicator_off",
      "no_late_text",
      "no_wrong_destination",
      "no_replay",
    ],
  })),
  {
    id: "outage",
    requires_origin: true,
    facts: [
      "failure_shown",
      "mic_indicator_off",
      "partial_not_inserted",
      "no_replay",
      "no_local_fallback",
    ],
  },
  {
    id: "no_replay",
    requires_origin: true,
    facts: ["no_reconnect", "no_second_audio_upload", "no_endpoint_switch"],
  },
  {
    id: "tls_invalid",
    requires_origin: true,
    facts: ["test_refuses", "no_authorization_delivered", "no_audio_delivered"],
  },
  {
    id: "ws_redirect",
    requires_origin: true,
    facts: [
      "socket_redirect_refused",
      "no_authorization_at_redirect_target",
      "no_audio_at_redirect_target",
    ],
  },
  {
    id: "finalization_stall",
    requires_origin: true,
    facts: [
      "deadline_error_shown",
      "mic_indicator_off",
      "partial_not_inserted",
      "cancel_available",
      "no_replay",
    ],
  },
  {
    id: "secure_token_storage",
    requires_origin: true,
    facts: [
      "secure_store_used",
      "no_token_in_url",
      "no_token_in_logs_or_exports",
      "no_token_in_task_text",
    ],
  },
];
const reasons = [
  "endpoint_unavailable",
  "device_unavailable",
  "build_unavailable",
  "credentials_unavailable",
  "fixture_unavailable",
  "instrumentation_unavailable",
];
export function origin(value) {
  if (value === undefined || value === null) return null;
  let url;
  try {
    url = new URL(value);
  } catch {
    throw fail(
      "usage",
      "Use an HTTPS root origin without a path or credentials.",
      2,
    );
  }
  if (
    url.protocol !== "https:" ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash ||
    value.length > 2048
  )
    throw fail(
      "usage",
      "Use an HTTPS root origin without a path, query, fragment or credentials.",
      2,
    );
  return url.origin;
}
export function template(revision, endpoint = null, now = new Date()) {
  if (!/^[a-f0-9]{40}$/.test(revision))
    throw fail(
      "usage",
      "--revision requires the exact full 40-character source commit SHA.",
      2,
    );
  return {
    version: 1,
    scope: "owner_device_observations",
    review_id: randomUUID(),
    code_revision: revision,
    created_at: now.toISOString(),
    endpoint: origin(endpoint),
    devices: DEVICES.map((device) => ({
      ...device,
      os_version: null,
      build_sha256: null,
      cases: CASES.map((test) => ({
        id: test.id,
        status: "pending",
        observed_at: null,
        evidence_sha256: null,
        reason: null,
        facts: Object.fromEntries(test.facts.map((key) => [key, null])),
      })),
    })),
  };
}
export async function initialize(config, { signal } = {}) {
  const value = template(config.revision, config.origin);
  canceled(signal);
  try {
    await writeFile(config.output, JSON.stringify(value, null, 2) + "\n", {
      flag: "wx",
      mode: 0o600,
      signal,
    });
  } catch {
    canceled(signal);
    throw fail(
      "output",
      "Choose a new private review file in an existing writable parent; existing files are never overwritten.",
    );
  }
  return {
    review_id: value.review_id,
    devices: DEVICES.length,
    pending: DEVICES.length * CASES.length,
    endpoint_set: value.endpoint !== null,
  };
}
function timestamp(value, now) {
  return (
    typeof value === "string" &&
    /^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{3}Z$/.test(
      value,
    ) &&
    Number.isFinite(Date.parse(value)) &&
    new Date(value).toISOString() === value &&
    Date.parse(value) <= now.getTime() + 300_000
  );
}
function digest(value) {
  return typeof value === "string" && /^[a-f0-9]{64}$/.test(value);
}
export function audit(value, now = new Date()) {
  const invalid = () => {
    throw fail(
      "review",
      "The review must match the strict private template; use documented device, case, fact, timestamp and evidence fields only.",
    );
  };
  if (
    !fields(value, [
      "version",
      "scope",
      "review_id",
      "code_revision",
      "created_at",
      "endpoint",
      "devices",
    ]) ||
    value.version !== 1 ||
    value.scope !== "owner_device_observations" ||
    !/^[a-f0-9]{8}-[a-f0-9]{4}-4[a-f0-9]{3}-[89ab][a-f0-9]{3}-[a-f0-9]{12}$/.test(
      value.review_id,
    ) ||
    !/^[a-f0-9]{40}$/.test(value.code_revision) ||
    !timestamp(value.created_at, now) ||
    !Array.isArray(value.devices) ||
    value.devices.length !== DEVICES.length
  )
    invalid();
  if (value.endpoint !== null && origin(value.endpoint) !== value.endpoint)
    invalid();
  const counts = { passed: 0, failed: 0, blocked: 0, pending: 0 },
    rows = [];
  for (let i = 0; i < DEVICES.length; i++) {
    const expected = DEVICES[i],
      device = value.devices[i];
    if (
      !fields(device, [
        "id",
        "model",
        "platform",
        "os_version",
        "build_sha256",
        "cases",
      ]) ||
      device.id !== expected.id ||
      device.model !== expected.model ||
      device.platform !== expected.platform ||
      !Array.isArray(device.cases) ||
      device.cases.length !== CASES.length
    )
      invalid();
    if (
      device.os_version !== null &&
      (typeof device.os_version !== "string" ||
        !/^[a-zA-Z0-9 ._()-]{1,64}$/.test(device.os_version))
    )
      invalid();
    if (device.build_sha256 !== null && !digest(device.build_sha256)) invalid();
    for (let j = 0; j < CASES.length; j++) {
      const test = CASES[j],
        result = device.cases[j];
      if (
        !fields(result, [
          "id",
          "status",
          "observed_at",
          "evidence_sha256",
          "reason",
          "facts",
        ]) ||
        result.id !== test.id ||
        !Object.hasOwn(counts, result.status) ||
        !fields(result.facts, test.facts) ||
        Object.values(result.facts).some(
          (fact) => fact !== null && typeof fact !== "boolean",
        )
      )
        invalid();
      if (result.status === "passed" || result.status === "failed") {
        if (
          !device.os_version ||
          !device.build_sha256 ||
          !timestamp(result.observed_at, now) ||
          Date.parse(result.observed_at) < Date.parse(value.created_at) ||
          !digest(result.evidence_sha256) ||
          result.reason !== null ||
          (test.requires_origin && value.endpoint === null) ||
          Object.values(result.facts).some((fact) => typeof fact !== "boolean")
        )
          invalid();
        const allTrue = Object.values(result.facts).every(
          (fact) => fact === true,
        );
        if ((result.status === "passed") !== allTrue) invalid();
      } else {
        if (
          result.observed_at !== null ||
          result.evidence_sha256 !== null ||
          Object.values(result.facts).some((fact) => fact !== null) ||
          (result.status === "pending"
            ? result.reason !== null
            : !reasons.includes(result.reason))
        )
          invalid();
      }
      counts[result.status]++;
      rows.push({ device: device.id, check: test.id, status: result.status });
    }
  }
  return {
    review_id: value.review_id,
    code_revision: value.code_revision,
    endpoint_set: value.endpoint !== null,
    ...counts,
    rows,
  };
}
export async function auditFile(file, { signal } = {}) {
  const bytes = await stableFile(file, 128 * 1024, { signal, collect: true });
  return {
    ...audit(json(bytes.bytes, "Private review")),
    review_sha256: bytes.sha256,
  };
}
