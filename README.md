# polyfrost-crowdin-relay

A Cloudflare Worker that forwards Crowdin webhooks to Discord as Components V2 messages.

Every event is posted to a global channel. Events that belong to a language (suggestions, approvals, comments, "fully translated" milestones) also go to that language's channel. Source string, file, build and pre-translation events only go to the global channel.

Batched webhooks are supported. A batch is grouped by event type, project, language and user, so 30 suggestions from one translator become one card with a few samples instead of 30 messages.

## Setup

1. Make a Discord webhook for the global channel and for each language channel.
2. Set the secrets:

   ```bash
   npx wrangler secret put RELAY_TOKEN
   npx wrangler secret put DISCORD_WEBHOOK_GLOBAL
   npx wrangler secret put DISCORD_WEBHOOK_FR
   npx wrangler secret put DISCORD_WEBHOOK_PT_BR
   ```

   Language secrets use the Crowdin language ID in upper case with `-` changed to `_`. `pt-BR` becomes `DISCORD_WEBHOOK_PT_BR`, `es-ES` becomes `DISCORD_WEBHOOK_ES_ES`. A language without a secret only shows up in the global channel.

3. Deploy:

   ```bash
   npx wrangler deploy
   ```

4. In each Crowdin project, open Tools → Webhooks and add a webhook:
   - URL: `https://<worker>/crowdin`
   - Request type: `POST`, content type `application/json`
   - Header: `X-Relay-Token: <RELAY_TOKEN>`
   - Turn on **Batch webhooks**
   - Pick the events you want

Both projects can point at the same URL. Every card names the project it came from.

## Local testing

Put the same secrets in `.dev.vars`, run `npx wrangler dev`, then post a payload:

```bash
curl -X POST http://localhost:8787/crowdin -H "X-Relay-Token: test" -H "Content-Type: application/json" -d @payload.json
```
