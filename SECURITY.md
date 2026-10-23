# Security and private data

Do not post API keys, OAuth codes, access/refresh tokens, passwords, library databases, or account exports in public issues. If a credential has been exposed, revoke it with the issuing service.

Credentials are accepted by the native app and protected with Windows DPAPI. Online services receive the credentials required for their own requests. This is a local desktop app, not an offline-only app.

## Publication boundary

Only reviewed source code, synthetic tests, original project graphics, and public documentation belong in this repository. Exclude:

- `.env` files, keys, tokens, credential exports, and account files
- SQLite databases and journals, library exports, and scraped metadata
- Downloaded cover art, backgrounds, screenshots of personal libraries, and provider caches
- Build output, executables, local tools, browser profiles, QA logs, and backups

The public Epic/GOG OAuth client constants implement the store protocols. They are not personal account credentials; account access still requires user authorization.

Report security issues without including exploit secrets or private user data. No dedicated private reporting channel is currently advertised; do not disclose a live credential in an issue.
