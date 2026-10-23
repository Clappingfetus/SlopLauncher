import { expect, it } from "vitest";
import { demoGames } from "./demo";
import { filterGames } from "./library";
import { searchIndex } from "./search-index";
it("finds accents, apostrophes, typographic dashes, genres and tags", () => {
  const game = {
    ...demoGames()[0],
    title: "Pokémon — Tom’s Adventure",
    genres: ["Rôle-playing"],
    tags: ["Coöperative"],
  };
  for (const query of [
    "pokemon",
    "pokémon",
    "toms adventure",
    "role playing",
    "cooperative",
  ])
    expect(filterGames([game], "all", query, false, "", "title")).toEqual([
      game,
    ]);
});
it("reuses cached text and invalidates it on renamed or retagged games", () => {
  const game = demoGames()[0],
    cached = searchIndex(game);
  expect(searchIndex(game)).toBe(cached);
  game.title = "Late arrival";
  expect(searchIndex(game)).not.toBe(cached);
  game.tags = ["Unique tag"];
  const tagged = searchIndex(game);
  game.tags.push("New tag");
  expect(searchIndex(game)).not.toBe(tagged);
  expect(filterGames([game], "all", "new tag", false, "", "title")).toEqual([
    game,
  ]);
  expect(filterGames([], "all", "new tag", false, "", "title")).toEqual([]);
});
