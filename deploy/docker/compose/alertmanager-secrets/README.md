# Slack webhooks for the Compose Alertmanager

Drop two files here (both git-ignored via `*.url`), each containing one Slack
incoming-webhook URL:

- `slack-pages.url`: channel for `severity=page` alerts (posted with `@here`)
- `slack-tickets.url`: channel for `severity=ticket` alerts

Using one channel for both? Put the same URL in both files. Then
`docker compose restart alertmanager`, or just wait: the files are read on each send.

Create a webhook: Slack → Apps → *Incoming Webhooks* → *Add to Slack* → pick the channel.
