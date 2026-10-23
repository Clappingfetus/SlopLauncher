import { describe, it, expect } from "vitest";
import { ratingRows, consensus, consensusScore } from "./ratings";
import { identity, EnrichmentQueue } from "./enrichment";
import { filterGames } from "./library";
import {
  defaultSettings,
  type Game,
  type ReviewRating,
  type ProviderId,
  type ProviderResult,
} from "./types";
import { versionLibrary } from "./versions";
const game = (id = "1"): Game => ({
  id,
  title: "Game",
  copies: [{ id, source: "steam", sourceId: "620", installed: false }],
  genres: [],
  screenshots: [],
  favorite: false,
  notes: "Keep me",
  status: "playing",
  addedAt: 1,
});
function add(
  g: Game,
  source: ReviewRating["source"],
  value: number,
  scale: number,
  count = 100,
) {
  const provider = `reviews-${source}` as ProviderId;
  return {
    ...g,
    enrichment: {
      ...g.enrichment,
      [provider]: {
        fingerprint: identity(g, provider),
        status: "ready" as const,
        checkedAt: 10,
        nextCheckAt: 99999,
        data: {
          rating: {
            source,
            kind:
              source === "opencritic"
                ? ("critics" as const)
                : ("users" as const),
            value,
            scale,
            count,
            url: "https://opencritic.com/game/1",
          },
        },
      },
    },
  };
}
describe("review consensus", () => {
  it("weights the user category 60/40, equally across user sources regardless of sample size", () => {
    let g = add(game(), "steam", 80, 100, 100000);
    g = add(g, "gog", 3, 5, 10);
    g = add(g, "metacritic", 7, 10);
    g = add(g, "itch", 4.5, 5);
    g = add(g, "opencritic", 90, 100);
    const score = consensus(ratingRows([g]))!;
    expect(score.userScore).toBe(7.5);
    expect(score.criticScore).toBe(9);
    expect(score.score).toBeCloseTo(8.1);
    expect(score.stars).toBe(8);
    expect(score.sources).toBe(5);
  });
  it("normalizes missing categories without zero penalties and rejects invalid samples", () => {
    expect(consensusScore(game())).toBeUndefined();
    expect(consensusScore(add(game(), "gog", 4.25, 5))).toBe(8.5);
    expect(consensusScore(add(game(), "opencritic", 85, 100))).toBe(8.5);
    for (const [value, scale, count] of [
      [NaN, 100, 10],
      [101, 100, 10],
      [2, 0, 10],
      [2, 5, 0],
      [-1, 5, 10],
    ])
      expect(
        consensusScore(add(game(), "gog", value, scale, count)),
      ).toBeUndefined();
    expect(consensusScore(add(game(), "metacritic", 0, 10))).toBe(1);
  });
  it("sorts by precise score in both directions, keeping unknown games last", () => {
    const a = add({ ...game("a"), title: "A" }, "metacritic", 8.21, 10);
    const b = add({ ...game("b"), title: "B" }, "metacritic", 8.24, 10);
    const unknown = game("c");
    expect(consensus(ratingRows([a]))?.stars).toBe(
      consensus(ratingRows([b]))?.stars,
    );
    expect(
      filterGames([a, unknown, b], "all", "", false, "", "rating-desc").map(
        (g) => g.id,
      ),
    ).toEqual(["b", "a", "c"]);
    expect(
      filterGames([a, unknown, b], "all", "", false, "", "rating-asc").map(
        (g) => g.id,
      ),
    ).toEqual(["a", "b", "c"]);
  });
  it("shares one rating per source across duplicate copies and preserves it across restart", () => {
    const a = add(game("a"), "steam", 80, 100);
    const b = add(
      {
        ...game("b"),
        copies: [
          { id: "gog", source: "gog", sourceId: "123", installed: false },
        ],
      },
      "gog",
      4.5,
      5,
    );
    const list = JSON.parse(JSON.stringify([a, b])) as Game[];
    const index = versionLibrary(list, defaultSettings);
    expect(consensusScore(index.withPresentation(list[0]))).toBe(8.5);
    expect(
      versionLibrary(list, defaultSettings).withPresentation(list[0]),
    ).toBe(index.withPresentation(list[0]));
    expect(consensusScore(index.withPresentation(list[1]))).toBe(8.5);
    expect(consensus(ratingRows([a, a, b]))?.sources).toBe(2);
    expect(list[0].notes).toBe("Keep me");
    expect(
      consensusScore(versionLibrary([a], defaultSettings).withPresentation(a)),
    ).toBe(8);
  });
  it("invalidates stale identities, review mappings and itch URLs", () => {
    const a = add(game(), "metacritic", 9, 10);
    expect(consensusScore({ ...a, title: "Other game" })).toBeUndefined();
    expect(
      consensusScore({ ...a, providerIds: { metacriticSlug: "other-game" } }),
    ).toBeUndefined();
    const wiki = {
      ...a,
      enrichment: {
        ...a.enrichment,
        pcgw: {
          fingerprint: identity(a, "pcgw"),
          status: "ready" as const,
          checkedAt: 1,
          nextCheckAt: 99999,
          data: { reviewLinks: { metacritic: "game" } },
        },
      },
    };
    expect(consensusScore(wiki)).toBeUndefined();
    expect(
      consensusScore({
        ...a,
        copies: [
          {
            ...a.copies[0],
            storePage: {
              source: "itch",
              sourceId: "1",
              url: "https://creator.itch.io/other",
            },
          },
        ],
      }),
    ).toBeUndefined();
  });
  it("handles late additions, ignores removed in-flight games, and retains saved scores on transient failure", async () => {
    let games: Game[] = [];
    let complete: ((r: ProviderResult) => void) | undefined;
    const queue = new EnrichmentQueue({
      getGames: () => games,
      enabled: (p) => p === "reviews-steam",
      now: () => 100,
      fetch: () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
      apply: (id, p, r) => {
        games = games.map((g) =>
          g.id === id ? { ...g, enrichment: { ...g.enrichment, [p]: r } } : g,
        );
      },
    });
    queue.reconcile(games);
    games = [game()];
    queue.reconcile(games);
    expect(complete).toBeDefined();
    games = [];
    complete!({ status: "ready", checkedAt: 100, nextCheckAt: 9999 });
    await new Promise((r) => setTimeout(r, 0));
    expect(games).toEqual([]);
    let old = add(game(), "steam", 80, 100);
    old.enrichment!["reviews-steam"]!.nextCheckAt = 1;
    games = [old];
    queue.reconcile(games);
    complete!({ status: "unavailable", checkedAt: 100, nextCheckAt: 9999 });
    await new Promise((r) => setTimeout(r, 0));
    expect(consensusScore(games[0])).toBe(8);
    expect(ratingRows(games)[0].stale).toBe(true);
    queue.clear();
  });
});
