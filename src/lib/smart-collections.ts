import type { GameCollection, SmartGroup } from "./types";
import { validateGroups } from "./grouping";
// Collections own their rules: overlapping matches never compete with another
// collection. Older saved collections keep their original ordered-rule semantics.
export function smartCollectionValue(
  id: string,
  name: string,
  group: SmartGroup,
  original?: GameCollection,
): GameCollection {
  const clean = name.trim();
  const previous = original?.rule?.mode === "smart" ? original.rule.groups : [];
  const rules = previous.length
    ? previous.map((g) => (g.id === group.id ? { ...group, name: g.name } : g))
    : [{ ...group, name: clean }];
  return {
    id,
    name: clean,
    gameIds: [],
    rule: {
      mode: "smart",
      groupId: `smart:${group.id}`,
      groups: validateGroups(rules),
    },
  };
}
export function legacySmartCollection(
  groups: SmartGroup[],
  index: number,
): GameCollection {
  const rules = validateGroups(groups.slice(0, index + 1));
  const group = rules[index];
  if (!group) throw Error("This saved rule is no longer available.");
  return {
    id: crypto.randomUUID(),
    name: group.name,
    gameIds: [],
    rule: { mode: "smart", groupId: `smart:${group.id}`, groups: rules },
  };
}
