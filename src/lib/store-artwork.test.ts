import { describe, expect, it } from "vitest";
import type { Game, Source } from "./types";
import { defaultSettings } from "./types";
import { needsAmazonPortraitRefresh, preferredArtwork } from "./store-artwork";
import { cleanAmazonTitle, reconcileStoreTitles } from "./store-titles";
import { mergeOwnedLibrary } from "./library";
import { presentGame, identity } from "./enrichment";
import { artworkIdentity, selectedArtwork } from "./artwork";
import { versionLibrary } from "./versions";

const urls = {
  steam: "https://cdn.akamai.steamstatic.com/steam/apps/1/cover.jpg",
  gog: "https://images.gog-statics.com/cover.jpg",
  epic: "https://cdn1.epicgames.com/cover.jpg",
  amazon: "https://m.media-amazon.com/images/I/cover.jpg",
  itch: "https://img.itch.zone/cover.jpg",
  grid: "https://cdn2.steamgriddb.com/grid/cover.jpg",
};
const game = (source: Source): Game => ({
  id: source,
  title: "Observer",
  copies: [
    {
      id: source + ":1",
      source,
      sourceId: "1",
      installed: false,
      ownership: { accountId: "fixture", verifiedAt: 10 },
    },
  ],
  cover: urls[source as keyof typeof urls],
  storeMetadata:
    source === "amazon"
      ? [
          {
            source,
            sourceId: "1",
            title: "Observer",
            cover: urls.amazon,
            coverKind: "portrait",
            checkedAt: 10,
          },
        ]
      : undefined,
  genres: [],
  screenshots: [],
  notes: "Keep",
  favorite: true,
  status: "playing",
  addedAt: 1,
});
describe("native store artwork", () => {
  it("rejects legacy Amazon banners and refreshes only unverified identities", () => {
    const legacy = { ...game("amazon"), storeMetadata: undefined };
    expect(preferredArtwork([legacy], "cover")).toEqual([]);
    expect(needsAmazonPortraitRefresh([legacy])).toBe(true);
    const imported = {
      ...game("amazon"),
      cover: undefined,
      storeMetadata: [
        { ...game("amazon").storeMetadata![0], cover: undefined },
      ],
    };
    const repaired = mergeOwnedLibrary([legacy], [imported], "amazon");
    expect(presentGame(repaired[0]).cover).toBeUndefined();
    expect(needsAmazonPortraitRefresh(repaired)).toBe(false);
    expect(
      needsAmazonPortraitRefresh([...repaired, { ...legacy, id: "late" }]),
    ).toBe(true);
    expect(
      needsAmazonPortraitRefresh([
        {
          ...game("amazon"),
          copies: [{ ...legacy.copies[0], sourceId: "changed" }],
        },
      ]),
    ).toBe(true);
    expect(
      preferredArtwork([{ ...legacy, hero: urls.amazon }], "hero"),
    ).toEqual([urls.amazon]);
    expect(preferredArtwork([legacy, game("gog")], "cover")).toEqual([
      urls.gog,
    ]);
  });

  it("uses the requested order across duplicates, independently of launch platform and display settings", () => {
    const games = [game("amazon"), game("epic"), game("gog"), game("steam")];
    expect(preferredArtwork(games, "cover")).toEqual([
      urls.steam,
      urls.gog,
      urls.epic,
      urls.amazon,
    ]);
    for (const hidden of [true, false]) {
      const index = versionLibrary(games, {
        ...defaultSettings,
        hideDuplicateGames: hidden,
        preferredSource: "amazon",
      });
      expect(index.withArtwork(games[0]).cover).toBe(urls.steam);
      expect(index.withArtwork(games[0]).copies[0].source).toBe("amazon");
      expect(index.withArtwork(games[0]).artworkFallbacks?.cover).toEqual([
        urls.gog,
        urls.epic,
        urls.amazon,
      ]);
    }
  });
  it("handles late copies, removal, changed source identity and preserves custom or locked art", () => {
    const amazon = game("amazon"),
      steam = game("steam");
    expect(
      versionLibrary([amazon], defaultSettings).withArtwork(amazon).cover,
    ).toBe(urls.amazon);
    expect(
      versionLibrary([amazon, steam], defaultSettings).withArtwork(amazon)
        .cover,
    ).toBe(urls.steam);
    expect(
      versionLibrary([amazon], defaultSettings).withArtwork(amazon).cover,
    ).toBe(urls.amazon);
    amazon.artwork = {
      cover: {
        url: urls.grid,
        source: "steamgriddb",
        identity: artworkIdentity(amazon),
        selectedAt: 1,
      },
    };
    const presented = presentGame(amazon);
    expect(
      versionLibrary([presented, steam], defaultSettings).withArtwork(presented)
        .cover,
    ).toBe(urls.grid);
    const locked = { ...amazon, metadataLocked: true, artwork: undefined };
    expect(
      versionLibrary([locked, steam], defaultSettings).withArtwork(locked),
    ).toBe(locked);
    const changed = {
      ...game("amazon"),
      storeMetadata: [
        {
          source: "amazon" as const,
          sourceId: "old",
          title: "Old",
          cover: urls.amazon,
          checkedAt: 1,
        },
      ],
    };
    expect(presentGame(changed).cover).toBeUndefined();
  });
  it("replaces old automatic grid art with native API art and retains it through a partial response", () => {
    const old = { ...game("amazon"), cover: urls.grid };
    const first = mergeOwnedLibrary([old], [game("amazon")], "amazon");
    expect(presentGame(first[0]).cover).toBe(urls.amazon);
    const next = mergeOwnedLibrary(
      first,
      [{ ...game("amazon"), cover: undefined }],
      "amazon",
    );
    expect(presentGame(next[0]).cover).toBe(urls.amazon);
    expect(next[0].notes).toBe("Keep");
    expect(next[0].favorite).toBe(true);
  });
  it("prefers a newer native record to an older raw image and uses itch native art before grid", () => {
    const gog = {
      ...game("gog"),
      storeMetadata: [
        {
          source: "gog" as const,
          sourceId: "1",
          title: "Observer",
          cover: "https://images.gog-statics.com/new.jpg",
          checkedAt: 10,
        },
      ],
    };
    expect(preferredArtwork([gog], "cover")[0]).toContain("/new.jpg");
    const itch = game("itch");
    itch.enrichment = {
      steamgriddb: {
        fingerprint: identity(itch, "steamgriddb"),
        status: "ready",
        checkedAt: 1,
        nextCheckAt: 99999,
        data: { cover: urls.grid },
      },
    };
    expect(preferredArtwork([itch], "cover")).toEqual([urls.itch, urls.grid]);
  });
});
describe("Amazon title corrections", () => {
  it("removes only promotional ranges for arbitrary titles, preserving real subtitles, dates and years", () => {
    for (const title of [
      "Observer: July 11 – 17",
      "Future Game: December 25 - January 3, 2027",
    ])
      expect(cleanAmazonTitle(title)).toBe(title.split(":")[0]);
    for (const title of [
      "The King of Fighters 2002",
      "Observer: System Redux",
      "July 11",
      "Game: July 11",
      "Game: July 42 – 99",
      ">observer_",
    ])
      expect(cleanAmazonTitle(title)).toBe(title);
  });
  it("repairs existing and late records and refreshes store titles without touching personal fields or overrides", () => {
    const old = { ...game("amazon"), storeMetadata: undefined, title: "Observer: July 11 – 17" };
    old.artwork = {
      cover: {
        identity: artworkIdentity(old),
        url: urls.grid,
        source: "steamgriddb",
        selectedAt: 1,
      },
    };
    const repaired = reconcileStoreTitles([old]);
    expect(repaired[0].title).toBe("Observer");
    expect(selectedArtwork(repaired[0], "cover")?.url).toBe(urls.grid);
    expect(reconcileStoreTitles(repaired)).toBe(repaired);
    expect(
      reconcileStoreTitles([{ ...old, titleOverride: true }])[0].title,
    ).toBe(old.title);
    expect(
      reconcileStoreTitles([{ ...old, metadataLocked: true }])[0].title,
    ).toBe(old.title);
    const imported = mergeOwnedLibrary([old], [game("amazon")], "amazon");
    const later = mergeOwnedLibrary(
      imported,
      [{ ...game("amazon"), title: "Observer: Complete Subtitle" }],
      "amazon",
    );
    expect(later[0].title).toBe("Observer: Complete Subtitle");
    expect(later[0].notes).toBe("Keep");
    expect(later[0].favorite).toBe(true);
    const custom = mergeOwnedLibrary(
      [{ ...old, titleOverride: true }],
      [game("amazon")],
      "amazon",
    );
    expect(custom[0].title).toBe(old.title);
  });
  it("only takes a fuller catalog title from a current verified match", () => {
    const g = game("amazon");
    g.enrichment = {
      igdb: {
        fingerprint: identity(g, "igdb", 7),
        status: "ready",
        checkedAt: 1,
        nextCheckAt: 999999,
        data: { id: 1, title: "Observer: Full Subtitle" },
      },
    };
    expect(reconcileStoreTitles([g], 7)[0].title).toBe(
      "Observer: Full Subtitle",
    );
    expect(
      reconcileStoreTitles([{ ...g, title: "New identity" }], 7)[0].title,
    ).toBe("New identity");
  });
});
