import { it, expect } from "vitest";
import {
  smartCollectionValue,
  legacySmartCollection,
} from "./smart-collections";
import {
  collectionMembers,
  saveCollection,
  resolveCollections,
} from "./collections";
import { defaultCondition } from "./grouping";
import { demoGames } from "./demo";
import type { SmartGroup } from "./types";
const rule = (name: string, tags: string): SmartGroup => ({
  id: crypto.randomUUID(),
  name,
  match: "all",
  conditions: [{ ...defaultCondition(), value: tags }],
});
it("saves independent Smart Collections, permits overlapping membership, and keeps personal data", () => {
  const game = {
    ...demoGames()[0],
    genres: ["Adventure", "RPG"],
    favorite: true,
    notes: "Keep",
  };
  const a = smartCollectionValue("a", "Adventures", rule("", "Adventure"));
  const b = smartCollectionValue("b", "Role playing", rule("", "RPG"));
  const saved = saveCollection(saveCollection([], a, [game]), b, [game]);
  expect(resolveCollections(saved, [game]).map((c) => c.gameIds)).toEqual([
    [game.id],
    [game.id],
  ]);
  expect(game.notes).toBe("Keep");
});
it("updates matches for late additions, changed tags, and removals after a settings roundtrip", () => {
  const first = { ...demoGames()[0], id: "first", genres: ["Adventure"] };
  const late = { ...demoGames()[1], id: "late", genres: ["RPG"] };
  const c = JSON.parse(
    JSON.stringify(smartCollectionValue("c", "RPG picks", rule("", "RPG"))),
  );
  expect(collectionMembers(c, [first])).toEqual([]);
  expect(collectionMembers(c, [first, late])).toEqual(["late"]);
  expect(collectionMembers(c, [{ ...late, genres: ["Puzzle"] }])).toEqual([]);
  expect(collectionMembers(c, [])).toEqual([]);
});
it("preserves old ordered-rule memberships when reusing or editing a saved rule", () => {
  const groups = [rule("Adventure", "Adventure"), rule("RPG", "RPG")];
  const shared = { ...demoGames()[0], genres: ["Adventure", "RPG"] };
  const rpg = { ...demoGames()[1], id: "rpg", genres: ["RPG"] };
  const old = legacySmartCollection(groups, 1);
  expect(collectionMembers(old, [shared, rpg])).toEqual(["rpg"]);
  const edited = smartCollectionValue(
    old.id,
    "Renamed",
    old.rule!.groups[1],
    old,
  );
  expect(collectionMembers(edited, [shared, rpg])).toEqual(["rpg"]);
  expect(groups[1].name).toBe("RPG");
});
it("rejects blank or incomplete rules and duplicate collection names", () => {
  expect(() => smartCollectionValue("a", "", rule("", "RPG"))).toThrow();
  expect(() =>
    smartCollectionValue("a", "Picks", { ...rule("", "RPG"), conditions: [] }),
  ).toThrow();
  const c = smartCollectionValue("a", "Picks", rule("", "RPG"));
  expect(() => saveCollection([c], { ...c, id: "b" }, [])).toThrow(
    /already exists/,
  );
});
