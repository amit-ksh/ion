export const VERSION = "0.1.0";
export const EVENT_LIMIT = 65_536;
export const FILE_LIMIT = 24 * 1024 * 1024;

export class AcceptanceError extends Error {
  constructor(code, message, exitCode = 1) {
    super(message);
    this.code = code;
    this.exitCode = exitCode;
  }
}
export function failure(code, message, exitCode = 1) {
  return new AcceptanceError(code, message, exitCode);
}
export function canceled(signal) {
  if (signal?.aborted)
    throw failure("canceled", "Speech check canceled; no audio is replayed.");
}
const booleanFlags = new Set(["--allow-loopback-http"]);
const commonFlags = [
  "--url",
  "--token-env",
  "--model",
  "--language",
  "--handshake-seconds",
  "--timeout-seconds",
  "--allow-loopback-http",
];
const acceptFlags = [
  "--wav",
  "--expected-text-file",
  "--clients",
  "--max-wer",
  "--max-audio-seconds",
  "--chunk-ms",
  "--finalize-seconds",
];
export const HELP = {
  check: [
    "Check readiness, inventory, HTTP/socket auth refusal and PCM16 negotiation without audio.",
    "node services/speech/accept.mjs check --url https://speech.example.com",
    "node services/speech/accept.mjs check --url http://127.0.0.1:8080 --allow-loopback-http",
    "Required: --url HTTPS root origin. Token: environment ION_SPEECH_TOKEN (or --token-env NAME).",
    "Optional: --model exact inventory ID (default Nemotron 3.5 transcription), --language auto,",
    "--handshake-seconds 10 (1..30), --timeout-seconds 180 (5..600), --allow-loopback-http.",
  ].join("\n"),
  accept: [
    "Run check, then stream the selected private WAV concurrently and verify final text and Cancel.",
    "node services/speech/accept.mjs accept --url https://speech.example.com --wav private.wav --expected-text-file private.txt",
    "node services/speech/accept.mjs accept --url https://speech.example.com --wav private.wav --expected-text-file private.txt --clients 2 --max-wer 0",
    "Required: --url, --wav mono PCM16 RIFF/WAVE (2..120 seconds), --expected-text-file private UTF8 text.",
    "Token: environment ION_SPEECH_TOKEN (or --token-env NAME). All check flags are supported.",
    "Optional: --clients 2 (1..4), --max-wer 0.25 (0..1), --max-audio-seconds 60 (2..120),",
    "--chunk-ms 100 (20..200), --finalize-seconds 30 (1..90). Sends one WAV per client and a Cancel prefix.",
    "Output contains counts, latency and word error rate; never audio, credentials or transcripts.",
  ].join("\n"),
};
function usage(message) {
  throw failure("usage", message, 2);
}
function integer(value, name, fallback, min, max) {
  if (value === undefined) return fallback;
  if (
    !/^[0-9]+$/.test(value) ||
    !Number.isSafeInteger(Number(value)) ||
    Number(value) < min ||
    Number(value) > max
  )
    usage(`${name} must be an integer from ${min} to ${max}.`);
  return Number(value);
}
export function parseArgs(argv) {
  const command = argv[0];
  if (!command) return { command: "home" };
  if (command === "--help") {
    if (argv.length !== 1)
      usage(
        "Use --help alone, or check --help / accept --help for scoped options.",
      );
    return { command: "help" };
  }
  if (command !== "check" && command !== "accept")
    usage("Use check or accept; run node services/speech/accept.mjs --help.");
  const known = new Set([
    ...commonFlags,
    ...(command === "accept" ? acceptFlags : []),
    "--help",
    "--version",
    "-v",
    "-V",
  ]);
  const values = new Map();
  for (let i = 1; i < argv.length; i++) {
    const flag = argv[i];
    if (!known.has(flag))
      usage(
        `Unrecognized input. Valid ${command} flags: ${[...known].join(", ")}.`,
      );
    if (values.has(flag)) usage(`Duplicate ${flag}; specify it once.`);
    if (
      flag === "--help" ||
      ["--version", "-v", "-V"].includes(flag) ||
      booleanFlags.has(flag)
    ) {
      values.set(flag, true);
      continue;
    }
    const value = argv[++i];
    if (value === undefined || value.startsWith("--"))
      usage(`${flag} requires a value.`);
    values.set(flag, value);
  }
  if (["--version", "-v", "-V"].some((flag) => values.has(flag)))
    return { command: "version" };
  if (values.has("--help")) return { command: "help", topic: command };
  if (!values.has("--url"))
    usage(
      "--url is required; supply the owner-controlled HTTPS speech root origin.",
    );
  const url = endpoint(
    values.get("--url"),
    values.has("--allow-loopback-http"),
  );
  const tokenEnv = values.get("--token-env") ?? "ION_SPEECH_TOKEN";
  if (!/^[A-Z][A-Z0-9_]{0,63}$/.test(tokenEnv))
    usage("--token-env must be an uppercase environment variable name.");
  const model = values.get("--model");
  if (
    model !== undefined &&
    (!model || model.length > 256 || /[^ -~]/.test(model))
  )
    usage(
      "--model must be a printable inventory ID of at most 256 characters.",
    );
  const language = values.get("--language") ?? "auto";
  if (!/^[A-Za-z][A-Za-z0-9_-]{0,31}$/.test(language))
    usage("--language must be a language code or auto.");
  const config = {
    command,
    url,
    tokenEnv,
    model,
    language,
    handshakeMs:
      1000 *
      integer(
        values.get("--handshake-seconds"),
        "--handshake-seconds",
        10,
        1,
        30,
      ),
    timeoutMs:
      1000 *
      integer(
        values.get("--timeout-seconds"),
        "--timeout-seconds",
        180,
        5,
        600,
      ),
  };
  if (command === "accept") {
    if (!values.has("--wav") || !values.has("--expected-text-file"))
      usage(
        "accept requires --wav and --expected-text-file; both stay private.",
      );
    const maxWerValue = values.get("--max-wer") ?? "0.25";
    if (!/^(?:0(?:\.[0-9]+)?|1(?:\.0+)?)$/.test(maxWerValue))
      usage("--max-wer must be a decimal from 0 to 1.");
    Object.assign(config, {
      wav: values.get("--wav"),
      expected: values.get("--expected-text-file"),
      maxWer: Number(maxWerValue),
      clients: integer(values.get("--clients"), "--clients", 2, 1, 4),
      maxAudioSeconds: integer(
        values.get("--max-audio-seconds"),
        "--max-audio-seconds",
        60,
        2,
        120,
      ),
      chunkMs: integer(values.get("--chunk-ms"), "--chunk-ms", 100, 20, 200),
      finalizeMs:
        1000 *
        integer(
          values.get("--finalize-seconds"),
          "--finalize-seconds",
          30,
          1,
          90,
        ),
    });
  }
  return config;
}
export function endpoint(value, allowLoopback = false) {
  let url;
  try {
    url = new URL(value);
  } catch {
    usage("Use an HTTPS speech root origin without a path or credentials.");
  }
  const localHttp =
    allowLoopback &&
    url.protocol === "http:" &&
    ["127.0.0.1", "[::1]"].includes(url.hostname);
  if (
    (!localHttp && url.protocol !== "https:") ||
    url.username ||
    url.password ||
    url.search ||
    url.hash ||
    url.pathname !== "/"
  )
    usage(
      "Use an HTTPS speech root origin without a path, query, fragment or credentials. HTTP requires --allow-loopback-http and a loopback IP.",
    );
  return url.origin;
}
export function tokenFromEnvironment(config, env = process.env) {
  const token = env[config.tokenEnv];
  if (
    typeof token !== "string" ||
    token.length < 32 ||
    token.length > 512 ||
    /[^!-~]/.test(token) ||
    /replace|placeholder|change[-_]?me|example|your[-_]?token/i.test(token)
  )
    usage(
      `Set ${config.tokenEnv} to a private random service token (32..512 printable nonspace characters); do not use a placeholder or pass the token in arguments.`,
    );
  return token;
}
