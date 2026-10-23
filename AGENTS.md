# Project rules

- The library is dynamic. Every per-game feature must handle newly discovered games, manual additions, imports, renamed/rematched games, removals, and new store adapters without editing a title list or running a one-off migration.
- Derive resource links from the current identity. Use the shared enrichment queue for external data, with persisted provenance, expiry/retry times, and identity-aware cache keys.
- Discard asynchronous results when the game is removed or its identity changes. Preserve personal notes, favorites, progress, launch settings, and locked metadata.
- Test late additions and identity changes, not only games present at startup. External failures must leave the library usable.
- HowLongToBeat and PCGamingWiki resources belong inside game details pages.
- Game resource links belong in the details action bar immediately before Favorite, without duplicate sidebar links. Steam community/store links require a Steam copy. Nexus links require a current verified catalog entry with published mods; never guess a game slug.
- Store API credentials only through the native protected credential boundary. Never put keys or client secrets in frontend persistence, logs, fixtures, or Git.
