import { describe, it, expect } from "vitest";
import { artworkIdentity, needsGridArtwork, selectedArtwork } from "./artwork";
import { EnrichmentQueue, identity, presentGame } from "./enrichment";
import { mergeDiscovered, mergeOwnedLibrary } from "./library";
import type { Game, ProviderId, ProviderResult } from "./types";
const game = (): Game => ({
  id: "new",
  title: "New game",
  copies: [{ id: "gog:42", source: "gog", sourceId: "42", installed: true }],
  cover: "https://images.gog-statics.com/original.jpg",
  hero: "https://images.gog-statics.com/original-wide.jpg",
  genres: [],
  screenshots: [],
  notes: "Keep my notes",
  favorite: true,
  status: "playing",
  addedAt: 1,
});
const attach = (g: Game, p: ProviderId, data: object, revision = 7) => {
  g.enrichment = {
    ...g.enrichment,
    [p]: {
      status: "ready",
      checkedAt: 1,
      nextCheckAt: 9999999999999,
      fingerprint: identity(g, p, revision),
      data,
    },
  };
};
const tick = async () => {
  for (let i = 0; i < 60; i++) await Promise.resolve();
};
describe("artwork defaults and deliberate choices", () => {
  it("keeps selected artwork valid when native JSON storage reorders provider keys", () => {
    const g=game();g.providerIds={steamgriddb:2,igdb:1};
    g.artwork={cover:{identity:artworkIdentity(g),url:"custom",source:"steamgriddb",selectedAt:1}};
    g.providerIds={igdb:1,steamgriddb:2};expect(selectedArtwork(g,"cover")?.url).toBe("custom");
  });
  it("restores saved store artwork over old automatic providers without rewriting the game", () => {
    const g = game();
    const original = structuredClone(g);
    attach(g, "steamgriddb", { cover: "grid", hero: "grid-wide" });
    attach(g, "igdb", { cover: "igdb", hero: "igdb-wide" });
    expect(presentGame(g, 7)).toMatchObject({
      cover: original.cover,
      hero: original.hero,
      notes: original.notes,
      favorite: true,
      status: "playing",
    });
    expect(g.cover).toBe(original.cover);
    expect(needsGridArtwork(g)).toBe(false);
  });
  it("fills PC gaps by slot while preserving an existing native background", () => {
    const g = game();
    g.cover = undefined;
    attach(g, "steamgriddb", { cover: "grid", hero: "grid-wide" });
    expect(needsGridArtwork(g)).toBe(true);
    expect(presentGame(g, 7)).toMatchObject({ cover: "grid", hero: g.hero });
  });
  it("does not automatically use SteamGridDB or IGDB images for emulated games", () => {
    const g = game();
    g.copies = [
      { id: "emu", source: "emulated", sourceId: "emu", installed: true },
    ];
    g.cover = undefined;
    g.hero = undefined;
    attach(g, "steamgriddb", {
      cover: "wrong-platform",
      hero: "wrong-platform",
    });
    attach(g, "igdb", { cover: "wrong-platform" });
    expect(needsGridArtwork(g)).toBe(false);
    expect(presentGame(g, 7).cover).toBeUndefined();
    expect(presentGame(g, 7).hero).toBeUndefined();
    attach(g, "screenscraper", { cover: "console-art" });
    expect(presentGame(g, 7).cover).toBe("console-art");
  });
  it("persists independent cover/background choices, including deliberate emulator artwork", () => {
    const g = game();
    g.copies = [
      { id: "emu", source: "emulated", sourceId: "emu", installed: true },
    ];
    g.metadataLocked = true;
    g.artwork = {
      cover: {
        identity: artworkIdentity(g),
        url: "chosen-cover",
        source: "steamgriddb",
        selectedAt: 1,
        author: "Artist",
        page: "https://www.steamgriddb.com/grid/1",
      },
      hero: {
        identity: artworkIdentity(g),
        url: "chosen-wide",
        source: "igdb",
        selectedAt: 1,
      },
    };
    const saved = JSON.parse(JSON.stringify(g));
    expect(presentGame(saved, 999)).toMatchObject({
      cover: "chosen-cover",
      hero: "chosen-wide",
      notes: g.notes,
      favorite: true,
    });
    delete saved.artwork.cover;
    expect(presentGame(saved, 999).cover).toBe(g.cover);
    expect(presentGame(saved, 999).hero).toBe("chosen-wide");
  });
  it("invalidates selections on rename, rematch and copy identity changes, but preserves personal edits", () => {
    const g = game();
    g.artwork = {
      cover: {
        identity: artworkIdentity(g),
        url: "custom",
        source: "steamgriddb",
        selectedAt: 1,
      },
    };
    g.notes = "New note";
    g.favorite = false;
    g.preferredCopy = "other";
    expect(selectedArtwork(g, "cover")?.url).toBe("custom");
    for (const change of [
      { title: "Renamed" },
      { metadataId: 42 },
      { providerIds: { steamgriddb: 42 } },
      { copies: [{ ...g.copies[0], sourceId: "99" }] },
    ]) {
      const changed = { ...g, ...change };
      expect(selectedArtwork(changed, "cover")).toBeUndefined();
      expect(presentGame(changed).cover).toBe(g.cover);
    }
  });
  it("keeps manual selections across scans, store refreshes and provider failures", () => {
    const g = game();
    g.artwork = {
      cover: {
        identity: artworkIdentity(g),
        url: "custom",
        source: "steamgriddb",
        selectedAt: 1,
      },
    };
    const incoming = {
      ...game(),
      cover: "new-native",
      copies: [
        { ...g.copies[0], ownership: { accountId: "account", verifiedAt: 2 } },
      ],
    };
    for (const merged of [
      mergeDiscovered([g], [incoming])[0],
      mergeOwnedLibrary([g], [incoming], "gog")[0],
    ]) {
      expect(merged.cover).toBe("new-native");
      expect(presentGame(merged).cover).toBe("custom");
      expect(merged.notes).toBe(g.notes);
    }
    g.metadataLocked = true;
    expect(mergeDiscovered([g], [incoming])[0].cover).toBe(g.cover);
  });
  it("queues late PC gaps, skips native and emulator images, and discards a result if a gap disappears", async () => {
    let games: Game[] = [];
    let release: ((value: ProviderResult) => void) | undefined;
    const fetched: string[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      enabled: (p, g) => p === "steamgriddb" && needsGridArtwork(g),
      fetch: (g) => {
        fetched.push(g.id);
        return new Promise((resolve) => (release = resolve));
      },
      apply: (id, p, result) => {
        games = games.map((g) =>
          g.id === id
            ? { ...g, enrichment: { ...g.enrichment, [p]: result } }
            : g,
        );
      },
    });
    queue.reconcile(games, 7);
    const native = game(),
      missing = { ...game(), id: "late", cover: undefined },
      emulated = {
        ...game(),
        id: "emu",
        cover: undefined,
        copies: [
          {
            id: "e",
            source: "emulated" as const,
            sourceId: "e",
            installed: true,
          },
        ],
      };
    games = [native, missing, emulated];
    queue.reconcile(games, 7);
    await tick();
    expect(fetched).toEqual(["late"]);
    games = games.map((g) =>
      g.id === "late" ? { ...g, cover: "arrived-from-store" } : g,
    );
    release!({
      status: "ready",
      checkedAt: 1,
      nextCheckAt: 9999999999999,
      data: { cover: "grid" },
    });
    await tick();
    expect(games.find((g) => g.id === "late")!.enrichment).toBeUndefined();
    expect(presentGame(games[1], 7).cover).toBe("arrived-from-store");
    queue.clear();
  });
});
