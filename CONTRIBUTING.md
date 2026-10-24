# Contributing

This project is in early development and is licensed under the [MIT License](LICENSE). Discuss substantial changes in an issue before preparing a contribution.

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

## Where to start in the code

| Area | Entry points | Important behavior |
| --- | --- | --- |
| UI and persistence | `src/App.svelte`, `src/lib/backend.ts` | UI edits are optimistic; ordered writes keep older snapshots from overwriting newer edits. Svelte snapshots already detach reactive data. |
| Library identity and versions | `src/lib/versions.ts`, `src/lib/duplicate-flags.ts` | Duplicate grouping is presentation-only. Keep original copies and reject conflicting catalog identities. Timestamp personal choices so later imports cannot restore old flags. |
| Metadata lifecycle | `src/lib/enrichment.ts`, `src/lib/enrichment-identity.ts` | Recheck identity and queue generation after asynchronous work. Cache results with provenance and retry times. |
| Artwork performance | `src/lib/artwork-loader.ts`, `src/lib/artwork-window.ts` | Visible users pin decoded entries; lookahead is bounded. Settle canceled promises and avoid decoding abandoned work. |
| Live Collections | `src/lib/grouping.ts`, `src/lib/collections.ts` | Evaluate against current games and time. Several collections can share a rule evaluation within one pass. |
| Store connections | `src-tauri/src/accounts.rs`, `src-tauri/src/itch.rs` | Native code owns account tokens. Failed or partial imports must not erase the library. |
| Credential protection | `src-tauri/src/providers.rs` | Secrets stay behind the native boundary. Never return them through status responses or errors. |

## Working safely with cached objects

Many presentation caches use the `Game` object as their key. Replace a changed game record instead of mutating it in place; retain unchanged objects so their caches remain warm. A matching title alone is not enough to accept an asynchronous result. Check the identity fingerprint and whether the game still exists.

Artwork requests have two lifetimes: a mounted card's acquired handle and the grid's prefetch window. Releasing a card must not cancel another card using the same image. Every acquired promise must eventually resolve or reject, including retries and cancellations.

Keep comments focused on invariants, ownership, and reasons for a decision. Avoid narrating obvious syntax or adding claims about performance without measurements. Add regression tests for the failure you fix, including late additions, removed games, changed matches, and unavailable providers when relevant.
