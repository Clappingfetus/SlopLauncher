import type { Game } from "./types";

// Fictional preview records: no scraped metadata or third-party artwork.
const titles = [
  "Aster Vale",
  "Quiet Orbit",
  "Paper Kingdom",
  "Northbound",
  "Mosslight",
  "Signal Coast",
  "Amber Roads",
  "Little Observatory",
  "Night Orchard",
  "Glass Harbor",
  "Wild Current",
  "Last Lantern",
];
export function demoGames(): Game[] {
  const now = Date.now();
  return titles.map((title, i) => ({
    id: `demo-${i + 1}`,
    title,
    copies: [
      {
        id: `demo-copy-${i + 1}`,
        source: (
          [
            "steam",
            "gog",
            "epic",
            "amazon",
            "steam",
            "gog",
            "manual",
            "gog",
            "steam",
            "epic",
            "steam",
            "steam",
          ] as const
        )[i],
        sourceId: String(900000000 + i),
        installed: i < 7,
      },
    ],
    cover: "/logo.svg",
    hero: "/preview-background.svg",
    description:
      "A fictional game used to preview SlopLauncher. Connect your own sources in the desktop app to populate your library.",
    developer: "Example Studio",
    genres: [i % 2 ? "Adventure" : "Exploration"],
    year: 2024 + (i % 3),
    screenshots: [],
    favorite: [0, 2, 6].includes(i),
    status: "unplayed",
    notes: "",
    addedAt: now - i * 86400000,
    lastPlayed: i < 5 ? now - i * 86400000 : undefined,
  }));
}
