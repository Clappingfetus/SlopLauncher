import type { Game, GameCollection, CollectionRule } from "./types";
import { collectGroupMemberIds, groupOptions, validateGroups } from "./grouping";
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
  if (!collection.rule) return collection.gameIds;
  const { mode, groups, groupId } = collection.rule;
  const members: string[] = [];
  collectGroupMemberIds(games, mode, groups, now, new Map([[groupId, members]]));
  return members;
}
export function resolveCollections(
  collections: GameCollection[],
  games: Game[],
  now = Date.now(),
): GameCollection[] {
  // Several live collections can select different buckets from the same rule
  // set. Collect only the requested buckets in one pass per rule set.
  const requests = new Map<
    string,
    { rule: CollectionRule; members: Map<string, string[]> }
  >();
  const resolved = collections.map((collection) => {
    const { rule } = collection;
    if (!rule) return collection;
    const key = JSON.stringify([rule.mode, rule.groups]);
    let request = requests.get(key);
    if (!request) {
      request = { rule, members: new Map() };
      requests.set(key, request);
    }
    let gameIds = request.members.get(rule.groupId);
    if (!gameIds) {
      gameIds = [];
      request.members.set(rule.groupId, gameIds);
    }
    return { ...collection, gameIds };
  });
  for (const { rule, members } of requests.values())
    collectGroupMemberIds(games, rule.mode, rule.groups, now, members);
  return resolved;
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
export function collectionCounts(
  collections: GameCollection[],
  isVisible: (id: string) => boolean,
): Map<string, number> {
  const counts = new Map<string, number>();
  for (const collection of collections) {
    const visible = new Set<string>();
    for (const id of collection.gameIds) {
      if (isVisible(id)) visible.add(id);
    }
    counts.set(collection.id, visible.size);
  }
  return counts;
}

// Both menu and drag actions validate current objects; a game or folder can
// disappear while a menu is open or a native drag gesture is in flight.
export function addCollectionGame(
  collections: GameCollection[],
  collectionId: string,
  gameId: string,
  games: Game[],
) {
  return addCollectionGames(collections, collectionId, [gameId], (id) =>
    games.some((g) => g.id === id),
  ).collections;
}

export function addCollectionGames(
  collections: GameCollection[],
  collectionId: string,
  gameIds: readonly string[],
  hasGame: (id: string) => boolean,
): { collections: GameCollection[]; addedCount: number } {
  if (!gameIds.length) return { collections, addedCount: 0 };
  const collection = collections.find((c) => c.id === collectionId);
  if (!collection || gameIds.some((id) => !hasGame(id)))
    throw Error("This game or collection is no longer available.");
  if (collection.rule)
    throw Error(
      "This collection follows automatic rules. Edit its rules to change the games it includes.",
    );
  if (gameIds.length === 1 && collection.gameIds.includes(gameIds[0]))
    return { collections, addedCount: 0 };
  const members = new Set(collection.gameIds);
  const originalSize = members.size;
  for (const id of gameIds) members.add(id);
  const addedCount = members.size - originalSize;
  return {
    collections: addedCount
      ? collections.map((c) =>
          c === collection ? { ...c, gameIds: [...members] } : c,
        )
      : collections,
    addedCount,
  };
}
