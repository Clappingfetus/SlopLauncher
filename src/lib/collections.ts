import type { Game, GameCollection, CollectionRule } from "./types";
import { groupGames, groupOptions, validateGroups } from "./grouping";
export function validateCollectionRule(rule: CollectionRule): CollectionRule {
  if (
    !rule ||
    rule.mode === ("none" as string) ||
    !groupOptions.some(([id]) => id === rule.mode) ||
    !rule.groupId ||
    rule.groupId.length > 256
  )
    throw Error("Invalid collection group rule.");
  const groups = rule.mode === "smart" ? validateGroups(rule.groups) : [];
  if (
    rule.mode === "smart" &&
    rule.groupId !== "other" &&
    !groups.some((g) => `smart:${g.id}` === rule.groupId)
  )
    throw Error("Collection group no longer exists.");
  return { mode: rule.mode, groupId: rule.groupId, groups };
}
export function collectionMembers(
  collection: GameCollection,
  games: Game[],
  now = Date.now(),
): string[] {
  return collection.rule
    ? (groupGames(games, collection.rule.mode, collection.rule.groups, now)
        .find((g) => g.id === collection.rule!.groupId)
        ?.games.map((g) => g.id) ?? [])
    : collection.gameIds;
}
export function resolveCollections(
  collections: GameCollection[],
  games: Game[],
  now = Date.now(),
): GameCollection[] {
  // Several live collections can select different buckets from the same rule
  // set. Evaluate that set once per resolution, but never cache across library
  // snapshots or clock changes (relative-date rules must keep moving).
  const matches = new Map<string, Map<string, string[]>>();
  return collections.map((c) => {
    if (!c.rule) return c;
    const key = JSON.stringify([c.rule.mode, c.rule.groups]);
    let buckets = matches.get(key);
    if (!buckets) {
      buckets = new Map(
        groupGames(games, c.rule.mode, c.rule.groups, now).map((g) => [
          g.id,
          g.games.map((game) => game.id),
        ]),
      );
      matches.set(key, buckets);
    }
    return { ...c, gameIds: buckets.get(c.rule.groupId) ?? [] };
  });
}
export type CollectionFilter = `collection:${string}`;
export function isCollectionFilter(value: string): value is CollectionFilter {
  return value.startsWith("collection:");
}
export function collectionName(
  name: string,
  collections: GameCollection[],
  id?: string,
) {
  const clean = name.trim();
  if (!clean || [...clean].length > 80 || /\p{Cc}/u.test(clean))
    throw new Error("Use a collection name of 1 to 80 characters.");
  if (
    collections.some(
      (c) =>
        c.id !== id &&
        c.name.normalize("NFKC").toLowerCase() ===
          clean.normalize("NFKC").toLowerCase(),
    )
  )
    throw new Error("A collection with this name already exists.");
  return clean;
}
export function saveCollection(
  collections: GameCollection[],
  input: GameCollection,
  games: Game[],
): GameCollection[] {
  const exists = collections.some((c) => c.id === input.id);
  if (!exists && collections.length >= 128)
    throw new Error("You can create up to 128 collections.");
  const live = new Set(games.map((g) => g.id));
  const value: GameCollection = {
    ...(input.rule ? { rule: validateCollectionRule(input.rule) } : {}),
    id: input.id,
    name: collectionName(input.name, collections, input.id),
    gameIds: input.rule
      ? []
      : [...new Set(input.gameIds)].filter((id) => live.has(id)),
  };
  return exists
    ? collections.map((c) => (c.id === value.id ? value : c))
    : [...collections, value];
}
export function setMembership(
  collections: GameCollection[],
  collectionId: string,
  gameId: string,
  included: boolean,
) {
  return collections.map((c) =>
    c.id !== collectionId || c.rule
      ? c
      : {
          ...c,
          gameIds: included
            ? [...new Set([...c.gameIds, gameId])]
            : c.gameIds.filter((id) => id !== gameId),
        },
  );
}
export function withoutGame(collections: GameCollection[], gameId: string) {
  return collections.map((c) =>
    c.gameIds.includes(gameId)
      ? { ...c, gameIds: c.gameIds.filter((id) => id !== gameId) }
      : c,
  );
}
export function collectionCounts(collections: GameCollection[], games: Game[]) {
  const live = new Set(games.map((g) => g.id));
  return new Map(
    collections.map((c) => [
      c.id,
      new Set(c.gameIds.filter((id) => live.has(id))).size,
    ]),
  );
}
