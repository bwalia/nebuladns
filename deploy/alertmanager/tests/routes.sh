#!/bin/sh
# Routing and Slack-template tests for ../alertmanager.yml. Needs amtool; CI runs it in
# the prom/alertmanager image:
#   docker run --rm -v "$PWD/deploy:/deploy:ro" --entrypoint sh \
#     prom/alertmanager:v0.27.0 /deploy/alertmanager/tests/routes.sh
set -eu

here="$(cd "$(dirname "$0")" && pwd)"
am="$here/.."
rules="$am/../prometheus/nebuladns.rules.yml"
cfg="$am/alertmanager.yml"
fails=0

route() {  # route <expected-receiver> <label=value>...
  want="$1"; shift
  got="$(amtool config routes test --config.file="$cfg" "$@")"
  if [ "$got" = "$want" ]; then
    echo "ok   $* -> $got"
  else
    echo "FAIL $* -> $got (want $want)"; fails=$((fails + 1))
  fi
}

echo "== every alert in the rules file routes by its severity"
# Pairs each `- alert:` with the `severity:` that follows it.
pairs="$(awk '/- alert:/ {a=$3} /severity:/ && a {print a, $2; a=""}' "$rules")"
[ -n "$pairs" ] || { echo "FAIL no alerts parsed from $rules"; exit 1; }
# The loop runs in a pipeline subshell, so it reports its own failure count via exit.
echo "$pairs" | {
  while read -r alert sev; do
    case "$sev" in
      page) route slack-pages alertname="$alert" severity=page job=nebuladns ;;
      ticket) route slack-tickets alertname="$alert" severity=ticket job=nebuladns ;;
      *) echo "FAIL $alert has unknown severity '$sev'"; fails=$((fails + 1)) ;;
    esac
  done
  exit "$fails"
} || fails=$((fails + 1))

echo "== anything without a known severity still reaches Slack"
route slack-tickets alertname=SomethingNew job=nebuladns

render() {  # render <template> <data-file>
  amtool template render --template.glob="$am/templates/*.tmpl" \
    --template.text="{{ template \"$1\" . }}" --template.data="$2"
}

expect() {  # expect <what> <haystack> <needle>
  case "$2" in
    *"$3"*) echo "ok   $1 contains: $3" ;;
    *) echo "FAIL $1 missing: $3"; echo "---"; echo "$2"; echo "---"; fails=$((fails + 1)) ;;
  esac
}

echo "== Slack message rendering"
firing="$here/firing-page.json"
resolved="$here/resolved-page.json"
expect "firing title" "$(render nebuladns.slack.title "$firing")" "[FIRING:2] NebulaDNSDown (page)"
expect "firing colour" "$(render nebuladns.slack.color "$firing")" "danger"
text="$(render nebuladns.slack.text "$firing")"
expect "firing text" "$text" "*NebulaDNS ns1:9090 is down* \`ns1:9090\`"
expect "firing text" "$text" "*NebulaDNS ns2:9090 is down* \`ns2:9090\`"
expect "firing text" "$text" "Prometheus has not scraped ns1:9090"
expect "runbook" "$(render nebuladns.slack.runbook "$firing")" \
  "https://github.com/bwalia/nebuladns/blob/main/docs/runbooks/NebulaDNSDown.md"
expect "silence" "$(render nebuladns.slack.silence "$firing")" \
  "http://alertmanager.example:9093/#/silences/new?filter=%7Balertname%3D%22NebulaDNSDown%22%2Cjob%3D%22nebuladns%22%2Cseverity%3D%22page%22%7D"
expect "resolved title" "$(render nebuladns.slack.title "$resolved")" "[RESOLVED] NebulaDNSDown (page)"
expect "resolved colour" "$(render nebuladns.slack.color "$resolved")" "good"
expect "resolved text" "$(render nebuladns.slack.text "$resolved")" "~NebulaDNS ns1:9090 is down~ resolved"

if [ "$fails" -ne 0 ]; then
  echo "$fails check(s) failed"; exit 1
fi
echo "all routing and template checks passed"
