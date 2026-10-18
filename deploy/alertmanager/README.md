# Alertmanager → Slack

Routes the alerts in [`../prometheus/nebuladns.rules.yml`](../prometheus/nebuladns.rules.yml)
to Slack.

| Severity | Receiver | Behaviour |
|---|---|---|
| `page` | `slack-pages` | `@here` while firing, re-posted hourly until resolved |
| `ticket` | `slack-tickets` | re-posted daily until resolved |
| anything else | `slack-tickets` | fallback, so nothing is dropped |

**Grouping.** Alerts are grouped by alert name and job: a fleet-wide outage is one post that
lists every instance, not one post per instance. Resolved alerts post in green without
`@here`.

**Inhibition:**
- `NebulaDNSAllInstancesDown` suppresses the per-instance `NebulaDNSDown`.
- `NebulaDNSFailoverBothPopsDown` suppresses `NebulaDNSFailoverPopDown`.

**Each post has:**
- a title like `[FIRING:2] NebulaDNSDown (page)`, linked to the alert's runbook section
- one bullet per instance, hostname or POP
- **Runbook** and **Silence** buttons; Silence opens Alertmanager with the alert's labels
  filled in

## 1. Create the webhooks

In Slack, go to **Apps → Incoming Webhooks → Add to Slack** and create one webhook per
channel, for example `#nebuladns-pages` and `#nebuladns-alerts`. A webhook only posts to
its own channel. To use one channel for both, reuse the same URL.

Webhook URLs are secrets. Never commit them; CI fails if one appears in a tracked file.

## 2. Install

**Docker Compose rig:** Alertmanager is already in the stack. Put the URLs in
`deploy/docker/compose/alertmanager-secrets/slack-pages.url` and `slack-tickets.url`
(git-ignored). Alertmanager UI: http://localhost:9093.

**VM hosts (plain Prometheus + Alertmanager):**
```bash
sudo install -d -m 0750 -o alertmanager /etc/alertmanager/templates /etc/alertmanager/secrets
sudo install -m 0644 deploy/alertmanager/alertmanager.yml /etc/alertmanager/
sudo install -m 0644 deploy/alertmanager/templates/*.tmpl /etc/alertmanager/templates/
printf '%s\n' 'https://hooks.slack.com/services/…' | sudo install -m 0600 -o alertmanager /dev/stdin /etc/alertmanager/secrets/slack-pages.url
printf '%s\n' 'https://hooks.slack.com/services/…' | sudo install -m 0600 -o alertmanager /dev/stdin /etc/alertmanager/secrets/slack-tickets.url
sudo systemctl reload alertmanager
```
Run Alertmanager with `--web.external-url=https://<its-public-url>`, because that's what
the Silence button links to. Point Prometheus at it:
```yaml
alerting:
  alertmanagers:
    - static_configs: [{ targets: ["127.0.0.1:9093"] }]
rule_files: [/etc/prometheus/rules/nebuladns.rules.yml]
```

**Kubernetes (kube-prometheus-stack):** pass the contents of `alertmanager.yml` as
`alertmanager.config` and the template as `alertmanager.templateFiles`. Two paths differ
there:
- Templates are mounted under `/etc/alertmanager/config/`, so change `templates:` to
  `/etc/alertmanager/config/*.tmpl`.
- A Secret listed in `alertmanager.alertmanagerSpec.secrets` is mounted at
  `/etc/alertmanager/secrets/<secret-name>/`, so change `api_url_file` accordingly.

## 3. Send a test alert

```bash
amtool alert add NebulaDNSTestAlert severity=page job=nebuladns instance=test:9090 \
  --annotation='summary="Test alert: please ignore"' \
  --annotation='runbook_url="https://github.com/bwalia/nebuladns/blob/main/docs/runbooks/NebulaDNSDown.md"' \
  --end="$(date -u -d '+2 min' +%Y-%m-%dT%H:%M:%SZ)" --alertmanager.url=http://localhost:9093
```
It posts to the pages channel within ~10 s, then as resolved after it expires. Use
`severity=ticket` to check the other channel. (macOS `date`: `-v+2M` instead of
`-d '+2 min'`.)

## Tests

`tests/routes.sh` runs in CI:
- checks that every alert in the rules file routes by its severity, so a new alert with
  an unknown severity fails
- renders the Slack templates against `tests/*.json` and checks the exact text
