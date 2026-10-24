# Set up SlopLauncher

1. Build and open the desktop app using the commands in the README.
2. Open **Sources**, or launch the skippable **Setup wizard** from Sources or Settings.
3. Connect the stores you use. Follow each source's in-app instructions; enter credentials only into the intended native setup flow. Steam requires your own API key and SteamID64, and itch.io accepts your personal API key. Other sources have their own account authorization flows.
4. Add installation folders as needed. Each source supports more than one folder. Owned-library imports and installed-game discovery are separate.
5. In **Settings**, optionally connect IGDB, SteamGridDB, or RetroAchievements. These services may require developer credentials or an API key. Credentials are not supplied with the project.
6. Add manual or emulated games with **Add game**. Configure emulator profiles first; provide your own emulator executable, game files, and RetroArch core where required.

## Make the library yours

Use **Group by → Smart groups** to combine rules. Save a group as a Collection and keep automatic matching enabled for a live Collection. Ratings support decimal thresholds such as 8.5. Unrated games are kept separate from numerical matches.

Enable **Hide duplicate games** and select a preferred platform in Settings. The version selector beside Play/Install overrides the default for a game. Favorites and hidden flags apply across matching copies. Hidden Games respects duplicate grouping.

Use the eye on a cover to hide a game, then restore it in **Hidden Games**. Settings also offers **Show hidden games**. Right-click covers for contextual actions; open a game's menu to change artwork, browse files, or request an uninstall through the supported platform flow.

## Metadata and artwork

Native store covers are preferred in this order: Steam, GOG, Epic, Amazon, then SteamGridDB. Per-game artwork choices take priority. Automatic SteamGridDB gap filling does not apply to emulated games by default. ScreenScraper integration is planned but not implemented yet.

Online lookup can be disabled in Settings. Missing matches or service failures do not remove your games. Manually correct a game's provider identity when necessary; data coverage is not guaranteed.

## Background updates

**Settings → Automatic library sync** checks connected stores every six hours by default. Choose hourly, every twelve hours, daily, or manual only. Successful sync times and one-hour failure cooldowns survive restarts. Use **Sync** in Sources for an immediate manual refresh.

This schedule is separate from **Automatically fill missing artwork & metadata**. Provider retry times remain saved, and newly added or rematched games are evaluated automatically. The bottom bar labels library syncs, metadata/resource checks, and active artwork downloads separately. Loading a cached image is not shown as a download.

## Large-library artwork

The launcher builds a persistent thumbnail store for library covers in the background. **Preparing covers** shows progress. Prepared covers are indexed as local paths, so scrolling beyond the decoded RAM cache does not require another provider request or thumbnail lookup. Full-resolution artwork still loads for visible cards and details.

Initial preparation requires time and disk space, especially when artwork must be downloaded. Visible image requests take priority. Small library thumbnails are kept separately from the disposable original-image cache and survive restarts; only a bounded set of decoded images stays in memory.

## Accounts and backups

Connections persist across restarts on the same Windows account. Back up the application data directory while the app is closed. Windows-protected credentials are tied to the user/machine context and should not be treated as a portable sign-in backup. Never publish this folder or send it with a bug report.

Store clients may need to be installed and signed in for launch/install actions. Games, ROMs, BIOS files, store accounts, and service credentials are not included.
