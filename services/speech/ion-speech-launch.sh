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
  --help) [ "$#" -eq 1 ] || ion_speech_fail 'Use --help alone.'; printf 'description: Launch the private loopback speech server after token and model checks.\nhelp: Set NEMO_SPEECH_HTTP_API_KEY and ION_SPEECH_MODEL_PATH; use --check for preflight or no arguments to serve.\n'; exit 0 ;;
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
if [ "${1:-}" = '--check' ]; then
  printf 'status: passed\nscope: launch_preflight\nchecks[3]{check,status}:\n  token,passed\n  binary,passed\n  model_file,passed\n'
  exit 0
fi
exec "$ion_speech_binary" --quiet serve --host 127.0.0.1 --port 8080 --asr-model "$ion_speech_model" --no-ui --max-upload-mb 50 --threads 4 --read-timeout 120 --write-timeout 120
