import { consensusScore } from "./ratings";
import type {
  Game,
  GroupMode,
  LibraryGroup,
  RuleField,
  SmartCondition,
  SmartGroup,
} from "./types";
import {
  DAY,
  GIB,
  calendarDay,
  installedBytes,
  mainExtraHours,
  releaseSpan,
} from "./game-facts";
import { tagKey } from "./tags";
export const groupOptions: [GroupMode, string][] = [
  ["none", "No grouping"],
  ["tags", "Primary tag"],
  ["size", "Installed size"],
  ["release", "Release year"],
  ["played", "Recently played"],
  ["added", "Date added"],
  ["hltb", "HLTB · Main + Extras"],
  ["rating", "Consensus rating"],
  ["smart", "Smart groups"],
];
export const fields: [RuleField, string][] = [
  ["tags", "Tags / genres"],
  ["size", "Installed size"],
  ["release", "Release date"],
  ["played", "Last played"],
  ["added", "Date added"],
  ["hltb", "HLTB · Main + Extras"],
  ["rating", "Consensus rating"],
];
export function operators(field: RuleField): [string, string][] {
  const presence: [string, string][] = [
    ["known", "Is known"],
    ["unknown", field === "played" ? "Never played / unknown" : "Is unknown"],
  ];
  if (field === "tags")
    return [
      ["hasAny", "Has any of"],
      ["hasAll", "Has all of"],
      ["hasNone", "Has none of"],
      ...presence,
    ];
  if (field === "size" || field === "hltb" || field === "rating")
    return [
      ["lt", "Less than"],
      ["lte", "At most"],
      ["gte", "At least"],
      ["gt", "More than"],
      ["between", "Between"],
      ...presence,
    ];
  const dates: [string, string][] = [
    ["before", "Before"],
    ["after", "After"],
    ["between", "Between"],
  ];
  return field === "release"
    ? [...dates, ...presence]
    : [
        ["within", "Within the last"],
        ["older", "More than ... ago"],
        ...dates,
        ...presence,
      ];
}
export function defaultCondition(field: RuleField = "tags"): SmartCondition {
  return {
    id: crypto.randomUUID(),
    field,
    op: operators(field)[0][0],
    value: "",
    upper: "",
  };
}
export function usesHltb(
  mode: GroupMode | undefined,
  groups: SmartGroup[] = [],
): boolean {
  return (
    mode === "hltb" ||
    (mode === "smart" &&
      groups.some((g) => g.conditions.some((c) => c.field === "hltb")))
  );
}
export function usesRatings(
  mode: GroupMode | undefined,
  groups: SmartGroup[] = [],
): boolean {
  return (
    mode === "rating" ||
    (mode === "smart" &&
      groups.some((g) => g.conditions.some((c) => c.field === "rating")))
  );
}
export function validateGroups(groups: SmartGroup[]): SmartGroup[] {
  if (!Array.isArray(groups) || groups.length > 32)
    throw Error("Use up to 32 smart groups.");
  const ids = new Set<string>(),
    names = new Set<string>();
  return groups.map((group) => {
    const name = group.name.trim();
    if (
      !group.id ||
      ids.has(group.id) ||
      !name ||
      name.length > 80 ||
      /\p{Cc}/u.test(name) ||
      names.has(name.normalize("NFKC").toLowerCase())
    )
      throw Error("Give each group a unique name of 1–80 characters.");
    ids.add(group.id);
    names.add(name.normalize("NFKC").toLowerCase());
    if (
      !["all", "any"].includes(group.match) ||
      !group.conditions.length ||
      group.conditions.length > 12
    )
      throw Error("Each group needs 1–12 conditions.");
    const conditionIds = new Set<string>();
    for (const c of group.conditions) {
      if (
        !c.id ||
        conditionIds.has(c.id) ||
        !fields.some(([f]) => f === c.field) ||
        !operators(c.field).some(([op]) => op === c.op)
      )
        throw Error(`Invalid condition in ${name}.`);
      conditionIds.add(c.id);
      if (["known", "unknown"].includes(c.op)) continue;
      if (c.field === "tags") {
        if (
          !c.value?.split(",").some((tag) => tagKey(tag)) ||
          c.value.length > 512
        )
          throw Error(`Enter tags for ${name}.`);
        continue;
      }
      const date =
        c.field === "release" ||
        ((c.field === "played" || c.field === "added") &&
          !["within", "older"].includes(c.op));
      const low = date
        ? calendarDay(c.value)
        : c.value?.trim()
          ? Number(c.value)
          : undefined;
      const high = date
        ? calendarDay(c.upper)
        : c.upper?.trim()
          ? Number(c.upper)
          : undefined;
      if (
        c.field === "rating" &&
        (low === undefined ||
          low < 1 ||
          low > 10 ||
          (c.op === "between" && (high === undefined || high < 1 || high > 10)))
      )
        throw Error(`Enter a consensus rating from 1 to 10 for ${name}.`);
      if (
        low === undefined ||
        !Number.isFinite(low) ||
        (!date && (low < 0 || low > 1_000_000))
      )
        throw Error(`Enter a valid ${date ? "date" : "number"} for ${name}.`);
      if (
        c.op === "between" &&
        (high === undefined ||
          !Number.isFinite(high) ||
          high < low ||
          (!date && high > 1_000_000))
      )
        throw Error(
          `Set an upper bound at least as large as the lower bound in ${name}.`,
        );
    }
    return {
      ...group,
      name,
      conditions: group.conditions.map((c) => ({ ...c })),
    };
  });
}
interface Facts {
  tags: Set<string>;
  size?: number;
  hltb?: number;
  rating?: number;
  release?: [number, number];
  played?: [number, number];
  added?: [number, number];
}
function facts(game: Game, now: number, needed: Set<RuleField>): Facts {
  const time = (value: number | undefined): [number, number] | undefined =>
    value && Number.isFinite(value) && value > 0 && value <= now
      ? [value, value]
      : undefined;
  const size = needed.has("size") ? installedBytes(game) : undefined;
  return {
    tags: new Set(
      needed.has("tags")
        ? [...game.genres, ...(game.tags ?? [])].map(tagKey).filter(Boolean)
        : [],
    ),
    size: size === undefined ? undefined : size / GIB,
    rating: needed.has("rating") ? consensusScore(game) : undefined,
    hltb: needed.has("hltb") ? mainExtraHours(game) : undefined,
    release: needed.has("release") ? releaseSpan(game) : undefined,
    played: needed.has("played") ? time(game.lastPlayed) : undefined,
    added: needed.has("added") ? time(game.addedAt) : undefined,
  };
}
function matches(f: Facts, c: SmartCondition, now: number): boolean {
  if (c.field === "tags") {
    if (c.op === "known") return f.tags.size > 0;
    if (c.op === "unknown") return !f.tags.size;
    const tags = (c.value ?? "").split(",").map(tagKey).filter(Boolean);
    if (!tags.length) return false;
    return c.op === "hasAny"
      ? tags.some((t) => f.tags.has(t))
      : c.op === "hasAll"
        ? tags.every((t) => f.tags.has(t))
        : c.op === "hasNone"
          ? tags.every((t) => !f.tags.has(t))
          : false;
  }
  const value = f[c.field];
  if (c.op === "unknown") return value === undefined;
  if (c.op === "known") return value !== undefined;
  if (value === undefined) return false;
  if (typeof value === "number") {
    const low = Number(c.value),
      high = Number(c.upper);
    if (!c.value?.trim() || !Number.isFinite(low)) return false;
    return c.op === "lt"
      ? value < low
      : c.op === "lte"
        ? value <= low
        : c.op === "gt"
          ? value > low
          : c.op === "gte"
            ? value >= low
            : c.op === "between" &&
              !!c.upper?.trim() &&
              Number.isFinite(high) &&
              value >= low &&
              value <= high;
  }
  const [start, end] = value;
  if (c.op === "within" || c.op === "older") {
    const days = Number(c.value);
    if (!c.value?.trim() || !Number.isFinite(days) || days < 0) return false;
    return c.op === "within"
      ? start >= now - days * DAY
      : end < now - days * DAY;
  }
  const low = calendarDay(c.value),
    high = calendarDay(c.upper);
  if (low === undefined) return false;
  return c.op === "before"
    ? end < low
    : c.op === "after"
      ? start >= low + DAY
      : c.op === "between" &&
        high !== undefined &&
        start >= low &&
        end < high + DAY;
}
export function groupGames(
  games: Game[],
  mode: GroupMode,
  rules: SmartGroup[] = [],
  now = Date.now(),
): LibraryGroup[] {
  if (mode === "none") return [];
  const groups = new Map<string, LibraryGroup>(),
    order = new Map<string, number>();
  const needed = new Set(
    rules.flatMap((g) => g.conditions.map((c) => c.field)),
  );
  const add = (id: string, name: string, game: Game, index: number) => {
    let group = groups.get(id);
    if (!group) {
      group = { id, name, games: [] };
      groups.set(id, group);
      order.set(id, index);
    }
    group.games.push(game);
  };
  for (const game of games) {
    if (mode === "smart") {
      const f = facts(game, now, needed);
      const index = rules.findIndex(
        (g) =>
          g.conditions?.length &&
          (g.match === "all"
            ? g.conditions.every((c) => matches(f, c, now))
            : g.match === "any" &&
              g.conditions.some((c) => matches(f, c, now))),
      );
      add(
        index < 0 ? "other" : `smart:${rules[index].id}`,
        index < 0 ? "Other" : rules[index].name,
        game,
        index < 0 ? rules.length : index,
      );
      continue;
    }
    if (mode === "tags") {
      const tag = [...game.genres, ...(game.tags ?? [])].find((t) => tagKey(t));
      add(
        `tag:${tag ? tagKey(tag) : "unknown"}`,
        tag ?? "Untagged",
        game,
        tag ? 0 : 1,
      );
      continue;
    }
    if (mode === "release") {
      const span = releaseSpan(game);
      const year = span ? new Date(span[0]).getUTCFullYear() : undefined;
      add(
        `year:${year ?? "unknown"}`,
        year ? String(year) : "Release date unknown",
        game,
        year ? -year : 1,
      );
      continue;
    }
    if (mode === "rating") {
      const score = consensusScore(game);
      const bucket =
        score === undefined
          ? 5
          : score >= 9
            ? 0
            : score >= 8
              ? 1
              : score >= 7
                ? 2
                : score >= 6
                  ? 3
                  : 4;
      add(
        `rating:${bucket}`,
        [
          "9–10 stars",
          "8–under 9 stars",
          "7–under 8 stars",
          "6–under 7 stars",
          "Under 6 stars",
          "Unrated",
        ][bucket],
        game,
        bucket,
      );
      continue;
    }
    if (mode === "hltb" || mode === "size") {
      const bytes = mode === "size" ? installedBytes(game) : undefined;
      const value =
        mode === "hltb"
          ? mainExtraHours(game)
          : bytes === undefined
            ? undefined
            : bytes / GIB;
      const limits = mode === "hltb" ? [10, 25, 50, 100] : [1, 10, 50, 100];
      const unit = mode === "hltb" ? "h" : "GiB";
      const index =
        value === undefined ? 5 : limits.findIndex((n) => value < n);
      const bucket = index === -1 ? 4 : index;
      const name =
        value === undefined
          ? mode === "hltb"
            ? "No HLTB time"
            : "Size unknown"
          : bucket === 0
            ? `Under ${limits[0]} ${unit}`
            : bucket === 4
              ? `${limits[3]}+ ${unit}`
              : `${limits[bucket - 1]}–${limits[bucket]} ${unit}`;
      add(`${mode}:${bucket}`, name, game, bucket);
      continue;
    }
    const date = mode === "played" ? game.lastPlayed : game.addedAt;
    const value =
      date && Number.isFinite(date) && date > 0 && date <= now
        ? date
        : undefined;
    const age = value === undefined ? undefined : (now - value) / DAY;
    const bucket =
      age === undefined
        ? 4
        : age <= 7
          ? 0
          : age <= 30
            ? 1
            : mode === "added" && age <= 365
              ? 2
              : 3;
    const name =
      bucket === 0
        ? "Last 7 days"
        : bucket === 1
          ? "Last 30 days"
          : bucket === 2
            ? "Last year"
            : bucket === 3
              ? "Earlier"
              : mode === "played"
                ? "Never played"
                : "Date added unknown";
    add(`${mode}:${bucket}`, name, game, bucket);
  }
  return [...groups.values()].sort(
    (a, b) =>
      order.get(a.id)! - order.get(b.id)! || a.name.localeCompare(b.name),
  );
}
