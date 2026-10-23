import { describe, expect, it } from "vitest";
import { demoGames } from "./demo";
import { resourceLinks, identity, EnrichmentQueue } from "./enrichment";
import { mergeDiscovered, mergeOwnedLibrary } from "./library";
import { storePage } from "./stores";
import type { Copy, Game, StorePage } from "./types";

function fixture(): Game {
  return {
    ...demoGames()[0],
    title: "A future game & sequel",
    enrichment: undefined,
    copies: [
      { id: "steam:42", source: "steam", sourceId: "42", installed: true },
      { id: "epic:abc", source: "epic", sourceId: "abc", installed: false },
      { id: "gog:123", source: "gog", sourceId: "123", installed: false },
      { id: "itch:456", source: "itch", sourceId: "456", installed: false },
    ],
  };
}
const pages: StorePage[] = [
  {
    source: "epic",
    sourceId: "abc",
    url: "https://store.epicgames.com/en-US/p/future-game",
  },
  {
    source: "gog",
    sourceId: "123",
    url: "https://www.gog.com/game/future_game",
  },
  {
    source: "itch",
    sourceId: "456",
    url: "https://creator.itch.io/future-game",
  },
];

describe("store links follow the active source", () => {
  it("switches product pages and Steam community links with the preferred copy", () => {
    const game = fixture();
    for (const copy of game.copies) {
      copy.storePage = pages.find((p) => p.source === copy.source);
      game.preferredCopy = copy.id;
      const links = resourceLinks(game);
      expect(links.store).toBe(
        copy.source === "steam"
          ? "https://store.steampowered.com/app/42/"
          : copy.storePage?.url,
      );
      expect(links.discussions !== undefined).toBe(copy.source === "steam");
      expect(links.guides !== undefined).toBe(copy.source === "steam");
      expect(links.storeName).toBe(
        {
          steam: "Steam Store",
          epic: "Epic Store",
          gog: "GOG Store",
          itch: "itch.io Store",
        }[copy.source as "steam"],
      );
    }
    game.copies = game.copies.filter((c) => c.source !== "itch");
    expect(resourceLinks(game).storeName).toBe("Steam Store");
  });
  it("uses the installed-copy fallback and never infers store ownership from metadata", () => {
    const game = fixture();
    game.copies = [
      { id: "manual", source: "manual", sourceId: "42", installed: true },
    ];
    expect(resourceLinks(game).store).toBeUndefined();
    game.copies[0].source = "amazon";
    expect(resourceLinks(game).store).toBeUndefined();
  });
  it("clearly labels searches when product metadata is missing and follows renames", () => {
    const game = fixture();
    for (const source of ["epic", "gog", "itch"] as const) {
      game.preferredCopy = `${source}:${source === "epic" ? "abc" : source === "gog" ? "123" : "456"}`;
      expect(resourceLinks(game).storeName).toMatch(/^Search /);
      const url = new URL(resourceLinks(game).store!);
      expect(url.searchParams.get(source === "gog" ? "query" : "q")).toBe(
        game.title,
      );
      game.title = "Renamed game";
      expect(resourceLinks(game).store).toContain("Renamed%20game");
    }
  });
  it("rejects stale identities, other stores, credentials, lookalikes and malformed product URLs", () => {
    const copy: Copy = {
      id: "itch:456",
      source: "itch",
      sourceId: "456",
      installed: false,
    };
    const page = pages[2];
    expect(storePage(copy, page)).toBe(page.url);
    expect(storePage({ ...copy, sourceId: "789" }, page)).toBeUndefined();
    expect(storePage({ ...copy, source: "epic" }, page)).toBeUndefined();
    for (const url of [
      "http://creator.itch.io/game",
      "https://creator.itch.io.evil.invalid/game",
      "https://nested.creator.itch.io/game",
      "https://user:pass@creator.itch.io/game",
      "https://creator.itch.io:1234/game",
      "https://creator.itch.io/game?download_key=private",
      "https://creator.itch.io/",
      "javascript:alert(1)",
    ])
      expect(storePage(copy, { ...page, url })).toBeUndefined();
  });
  it("only consumes current cached GOG product results", () => {
    const game = fixture();
    game.preferredCopy = "gog:123";
    game.enrichment = {
      stores: {
        status: "ready",
        checkedAt: 1,
        nextCheckAt: 2,
        fingerprint: identity(game, "stores"),
        data: { storePages: [pages[1]] },
      },
    };
    expect(resourceLinks(game).store).toBe(pages[1].url);
    game.copies[2].sourceId = "999";
    expect(resourceLinks(game).storeName).toBe("Search GOG Store");
  });
  it("retains product metadata through scans and account sync without changing personal or launch fields", () => {
    const game = fixture();
    game.notes = "Keep me";
    game.favorite = true;
    game.copies = [
      {
        ...game.copies[1],
        installed: true,
        executable: "C:\\games\\game.exe",
        storePage: pages[0],
      },
    ];
    game.preferredCopy = game.copies[0].id;
    const incoming: Game = {
      ...game,
      copies: [
        {
          ...game.copies[0],
          installed: false,
          executable: undefined,
          storePage: undefined,
          ownership: { accountId: "qa", verifiedAt: 1 },
        },
      ],
    };
    expect(mergeDiscovered([game], [incoming])[0].copies[0].storePage).toEqual(
      pages[0],
    );
    const refreshed = {
      ...pages[0],
      url: "https://store.epicgames.com/en-US/p/new-product",
    };
    incoming.copies[0].storePage = refreshed;
    const merged = mergeOwnedLibrary([game], [incoming], "epic")[0];
    expect(merged).toMatchObject({
      notes: "Keep me",
      favorite: true,
      preferredCopy: game.preferredCopy,
    });
    expect(merged.copies[0]).toMatchObject({
      installed: true,
      executable: "C:\\games\\game.exe",
      storePage: refreshed,
    });
    expect(game.copies[0].storePage).toEqual(pages[0]);
  });
  it("resolves late GOG additions and discards in-flight results after rematches or removal", async () => {
    let games: Game[] = [];
    const pending: (() => void)[] = [];
    const queue = new EnrichmentQueue({
      getGames: () => games,
      enabled: (p) => p === "stores",
      fetch: async () => {
        await new Promise<void>((resolve) => pending.push(resolve));
        return {
          status: "ready",
          checkedAt: Date.now(),
          nextCheckAt: Date.now() + 60000,
          data: { storePages: [pages[1]] },
        };
      },
      apply: (id, provider, value) => {
        games = games.map((g) =>
          g.id === id
            ? { ...g, enrichment: { ...g.enrichment, [provider]: value } }
            : g,
        );
      },
    });
    const flush = async () => {
      for (let i = 0; i < 50; i++) await Promise.resolve();
    };
    queue.reconcile(games);
    const game = fixture();
    game.preferredCopy = "gog:123";
    games = [game];
    queue.reconcile(games);
    await flush();
    pending.splice(0).forEach((f) => f());
    await flush();
    expect(resourceLinks(games[0]).store).toBe(pages[1].url);
    games = [
      {
        ...games[0],
        copies: games[0].copies.map((c) =>
          c.source === "gog" ? { ...c, sourceId: "999" } : c,
        ),
      },
    ];
    queue.reconcile(games);
    await flush();
    games = [{ ...games[0], title: "A rematched game" }];
    pending.splice(0).forEach((f) => f());
    await flush();
    expect(resourceLinks(games[0]).storeName).toBe("Search GOG Store");
    games = [];
    queue.reconcile(games);
    pending.splice(0).forEach((f) => f());
    await flush();
    expect(games).toEqual([]);
    queue.clear();
  });
});
