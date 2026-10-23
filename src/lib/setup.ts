export const setupServices = [
  {
    id: "steam",
    name: "Steam",
    purpose: "Owned games and Steam achievements",
    steps: [
      "Choose Connect. Use Find my SteamID64 to open your Steam account page and copy the 17-digit ID.",
      "Open Get an API key, sign in to Steam, and copy your Web API key into the connection form.",
      "Set Steam profile Game details visibility to Public if your owned library is missing. Choose Connect and import.",
    ],
    tip: "Use the numeric SteamID64, not your display name or custom profile URL.",
  },
  {
    id: "epic",
    name: "Epic Games",
    purpose: "Your full Epic library",
    steps: [
      "Choose Connect to open Epic sign-in in your browser.",
      "Finish sign-in and any verification. Copy the authorizationCode value or the full JSON shown at the end.",
      "Return to the launcher, paste it into the connection form, and choose Connect and import.",
    ],
    tip: "If the temporary code expires, start sign-in again. Your password stays in the browser.",
  },
  {
    id: "amazon",
    name: "Amazon Games",
    purpose: "Claimed Amazon games",
    steps: [
      "Choose Connect and sign in to the Amazon account that owns your games.",
      "Copy the final browser address containing openid.oa2.authorization_code.",
      "Paste that full address into the launcher and choose Connect and import.",
    ],
    tip: "Use the account with the claimed games. Prime offers must be claimed before they belong to your library.",
  },
  {
    id: "gog",
    name: "GOG",
    purpose: "Your full GOG library",
    steps: [
      "Choose Connect and sign in to GOG in your browser.",
      "Copy the final browser address containing code=.",
      "Paste that address into the launcher and choose Connect and import.",
    ],
    tip: "An account imports ownership. Local folders help the launcher find installed copies.",
  },
  {
    id: "itch",
    name: "itch.io",
    purpose: "Purchases, claims and owned bundles",
    steps: [
      "Choose Connect, then Get an API key to open itch.io account settings.",
      "Create or copy your personal API key and paste it into the launcher.",
      "Choose Connect and import. Use the official itch app to install and launch supported purchases.",
    ],
    tip: "A download folder alone cannot reveal your full owned library.",
  },
  {
    id: "steamgriddb",
    name: "SteamGridDB",
    purpose: "Fill missing PC covers and choose custom artwork",
    steps: [
      "Open Get an API key and sign in to SteamGridDB.",
      "Copy your API key into the field below, then choose Save provider settings.",
      "Use Test connection after saving. Change Artwork in a game’s menu lets you choose artwork manually.",
    ],
    tip: "Native store covers stay first. Automatic gap filling excludes emulated games.",
  },
  {
    id: "igdb",
    name: "IGDB",
    purpose: "Descriptions, release dates, genres and more",
    steps: [
      "Open Twitch applications and register an application with your Twitch account (two-factor authentication is required).",
      "Choose Confidential as the client type; IGDB’s guide uses localhost for the redirect URL. Open Manage to copy the Client ID and generate a Client Secret.",
      "Paste both values below, save, then test the connection.",
    ],
    tip: "Use an application client secret, not your Twitch password. Generating a new secret invalidates the old one.",
  },
  {
    id: "screenscraper",
    name: "ScreenScraper",
    purpose: "Console metadata and box art",
    steps: [
      "Open API access & documentation to arrange ScreenScraper developer API access.",
      "Enter your developer ID and developer password. A regular website account is not enough.",
      "Optionally add your personal login, save, then test. Configure emulator profiles and add your games separately.",
    ],
    tip: "Developer credentials and your optional personal account are different credentials.",
  },
  {
    id: "retroachievements",
    name: "RetroAchievements",
    purpose: "Regular and Hardcore achievement progress",
    steps: [
      "Open Get RetroAchievements API key and sign in to your account control panel.",
      "Enter your username and Web API key, save, then test.",
      "In an emulated game’s details, link its exact RetroAchievements game URL or ID. Sign in inside a compatible emulator to earn achievements.",
    ],
    tip: "The launcher displays progress; emulator support and the correct game version determine whether unlocks can be earned.",
  },
  {
    id: "emulators",
    name: "Emulators",
    purpose: "Launch ROMs through your installed emulators",
    steps: [
      "Install and configure your emulator, then choose Add profile below.",
      "Select the console, emulator and its executable file. For RetroArch, also choose an installed core DLL for that console.",
      "Save the profile. Use Add game → Emulated game to select a game file and its profile.",
    ],
    tip: "The executable is the emulator program, not the ROM. Keep the preset arguments unless your emulator needs different ones.",
  },
] as const;
export type SetupId = (typeof setupServices)[number]["id"];
export type ProviderSetupId =
  "steamgriddb" | "igdb" | "screenscraper" | "retroachievements";
