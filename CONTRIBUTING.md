# Contributing

This project is in early development and has no selected open-source license yet. Discuss substantial changes in an issue before preparing a contribution.

## Useful bug reports

Include app version, Windows version, affected source, expected behavior, actual behavior, and reproduction steps. Use a fictional or redacted example. Never include credentials, personal library databases, or cached provider payloads.

## Engineering principles

- Keep the library usable when external services fail.
- Handle new games, changed identities, removals, and new store adapters dynamically.
- Put external lookups through the shared enrichment queue and reject stale results.
- Preserve user notes, favorites, hidden state, progress, and explicit overrides.
- Keep credentials inside the native protected boundary.
- Validate behavior with late additions and rematched identities.

Run `npm run check`, `npm test`, and relevant native tests before submitting changes. Use synthetic fixtures and original placeholder art; do not commit scraped payloads or downloaded covers.
