import { expect, it } from "vitest";
import { demoGames } from "./demo";
import { narrowGameSearch, searchIndex, searchWords } from "./search-index";

it("keeps match order across narrowing, backspacing, and changing terms", () => {
  const example = demoGames()[0];
  const games = [
    { ...example, title: "Pokémon — Tom’s Adventure" },
    { ...example, title: "Pokémon Stadium" },
    { ...example, title: "Tom's Trail" },
  ];
  const search = narrowGameSearch(
    games.map((game) => ({ game, text: searchIndex(game).text })),
  );
  const matching = (query: string) => search(searchWords(query));

  expect(matching("poke")).toEqual(games.slice(0, 2));
  const unique = matching("pokemon tom");
  expect(unique).toEqual([games[0]]);
  expect(matching("toms pokémon")).toBe(unique);
  expect(matching("toms pokémon missing")).toEqual([]);
  expect(matching("toms")).toEqual([games[0], games[2]]);
  expect(matching("pokemon")).toEqual(games.slice(0, 2));
});

it("rechecks the full scope when dropping terms and reads the current game snapshots", () => {
  const example = demoGames()[0];
  const games = [
    { ...example, id: "a", title: "Clockwork Harbor", tags: ["Adventure"] },
    { ...example, id: "b", title: "Harbor Lights", tags: ["Simulation"] },
    { ...example, id: "c", title: "Clockwork Forest", tags: ["Adventure"] },
  ];
  const createSearch = (scope: typeof games) =>
    narrowGameSearch(scope.map((game) => ({ game, text: searchIndex(game).text })));
  const initial = createSearch(games);
  const harbor = initial(searchWords("harbor harbor"));
  expect(harbor).toEqual(games.slice(0, 2));
  expect(initial(searchWords("Harbor"))).toBe(harbor);
  expect(initial(searchWords("harbor adventure"))).toEqual([games[0]]);
  expect(initial(searchWords("harbor")).map((game) => game.id)).toEqual(["a", "b"]);
  expect(initial([])).toEqual(games);

  const late = { ...example, id: "d", title: "The Harbor", tags: ["Adventure"] };
  const rematched = { ...games[2], title: "Clockwork Docks", tags: ["Simulation"] };
  const updated = createSearch([games[1], rematched, late]);
  expect(updated(searchWords("harbor adventure"))).toEqual([late]);
  expect(updated(searchWords("clockwork simulation"))).toEqual([rematched]);
  expect(updated(searchWords("forest"))).toEqual([]);
});
