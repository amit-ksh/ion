#!/bin/sh
# The token stays in the environment; it is never an argument or a diagnostic.
set -eu
LC_ALL=C
export LC_ALL
ion_speech_fail() {
  printf 'status: failed\nerror: %s\n' "$1"
  exit 1
}
case "${1:-}" in
  -v|-V|--version) [ "$#" -eq 1 ] || ion_speech_fail 'Use --version alone.'; printf '0.1.0\n'; exit 0 ;;
  --help) [ "$#" -eq 1 ] || ion_speech_fail 'Use --help alone.'; printf 'description: Launch the private loopback speech server after token and model checks.\nhelp: Set NEMO_SPEECH_HTTP_API_KEY, ION_SPEECH_MODEL_PATH and ION_SPEECH_MODEL_SHA256; use --check for preflight or no arguments to serve.\n'; exit 0 ;;
  --check) [ "$#" -eq 1 ] || ion_speech_fail 'Use --check alone.' ;;
  '') [ "$#" -eq 0 ] || ion_speech_fail 'No empty or additional arguments are accepted.' ;;
  *) ion_speech_fail 'Unknown arguments. Use --check, --help or --version, or no arguments to serve.' ;;
esac
ion_speech_token=${NEMO_SPEECH_HTTP_API_KEY:-}
[ "${#ion_speech_token}" -ge 32 ] && [ "${#ion_speech_token}" -le 512 ] || ion_speech_fail 'Set a private random speech token of 32 to 512 characters.'
case "$ion_speech_token" in
  *[!!-~]*|*replace*|*REPLACE*|*placeholder*|*PLACEHOLDER*|*changeme*|*CHANGEME*|*example*|*EXAMPLE*) ion_speech_fail 'Replace the placeholder with a private printable token without spaces.' ;;
esac
ion_speech_binary=${ION_SPEECH_BINARY:-/opt/ion-speech/bin/nemo-speech}
case "$ion_speech_binary" in /*) ;; *) ion_speech_fail 'Use an absolute verified speech binary path.' ;; esac
[ -f "$ion_speech_binary" ] && [ -x "$ion_speech_binary" ] || ion_speech_fail 'Install a verified executable speech server binary.'
ion_speech_model=${ION_SPEECH_MODEL_PATH:-}
case "$ion_speech_model" in /*.gguf) ;; *) ion_speech_fail 'Set ION_SPEECH_MODEL_PATH to an absolute verified GGUF file.' ;; esac
[ -f "$ion_speech_model" ] && [ -r "$ion_speech_model" ] || ion_speech_fail 'Pull and select the verified Nemotron 3.5 GGUF model as the service user.'
ion_speech_expected_sha=${ION_SPEECH_MODEL_SHA256:-}
[ "${#ion_speech_expected_sha}" -eq 64 ] || ion_speech_fail 'Set ION_SPEECH_MODEL_SHA256 to the expected pinned conversion checksum.'
case "$ion_speech_expected_sha" in *[!a-fA-F0-9]*) ion_speech_fail 'The expected model SHA-256 must contain exactly 64 hexadecimal characters.' ;; esac
command -v sha256sum >/dev/null 2>&1 && command -v tr >/dev/null 2>&1 || ion_speech_fail 'Install the standard sha256sum and tr utilities for model verification.'
ion_speech_expected_sha=$(printf '%s' "$ion_speech_expected_sha" | tr 'A-F' 'a-f')
ion_speech_actual_sha=$(sha256sum 2>/dev/null < "$ion_speech_model") || ion_speech_fail 'The selected model could not be hashed.'
ion_speech_actual_sha=${ion_speech_actual_sha%% *}
[ "$ion_speech_actual_sha" = "$ion_speech_expected_sha" ] || ion_speech_fail 'The selected model does not match the configured pinned conversion checksum.'
if [ "${1:-}" = '--check' ]; then
  printf 'status: passed\nscope: launch_preflight\nchecks[4]{check,status}:\n  token,passed\n  binary,passed\n  model_file,passed\n  model_sha256,passed\n'
  exit 0
fi
exec "$ion_speech_binary" --quiet serve --host 127.0.0.1 --port 8080 --asr-model "$ion_speech_model" --asr.model.name nemotron-3.5 --no-ui --max-upload-mb 50 --threads 4 --read-timeout 120 --write-timeout 120
