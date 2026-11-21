import { lastPlayedAt, recentSortAt } from "./play-activity";
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
  ["smart", "Smart Collections"],
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
  tags?: Set<string>;
  size?: number;
  hltb?: number;
  rating?: number;
  release?: [number, number];
  played?: [number, number];
  added?: [number, number];
}
function facts(
  game: Game,
  now: number,
  needed: Set<RuleField>,
  normalizedTag: (raw: string) => string,
): Facts {
  const time = (value: number | undefined): [number, number] | undefined =>
    value && Number.isFinite(value) && value > 0 && value <= now
      ? [value, value]
      : undefined;
  const size = needed.has("size") ? installedBytes(game) : undefined;
  let tags: Set<string> | undefined;
  if (needed.has("tags")) {
    tags = new Set();
    for (const raw of game.genres) {
      const key = normalizedTag(raw);
      if (key) tags.add(key);
    }
    for (const raw of game.tags ?? []) {
      const key = normalizedTag(raw);
      if (key) tags.add(key);
    }
  }
  return {
    tags,
    size: size === undefined ? undefined : size / GIB,
    rating: needed.has("rating") ? consensusScore(game) : undefined,
    hltb: needed.has("hltb") ? mainExtraHours(game) : undefined,
    release: needed.has("release") ? releaseSpan(game) : undefined,
    played: needed.has("played") ? time(lastPlayedAt(game)) : undefined,
    added: needed.has("added") ? time(game.addedAt) : undefined,
  };
}
interface CompiledCondition {
  c: SmartCondition;
  tags?: string[];
  numLow?: number;
  numHigh?: number;
  hasNumLow?: boolean;
  hasNumHigh?: boolean;
  dayLow?: number;
  dayHigh?: number;
}
function compileCondition(c: SmartCondition): CompiledCondition {
  if (c.field === "tags") {
    return {
      c,
      tags:
        c.op === "known" || c.op === "unknown"
          ? undefined
          : (c.value ?? "").split(",").map(tagKey).filter(Boolean),
    };
  }
  const hasNumLow = !!c.value?.trim();
  const numLow = hasNumLow ? Number(c.value) : NaN;
  const hasNumHigh = !!c.upper?.trim();
  const numHigh = hasNumHigh ? Number(c.upper) : NaN;
  const needsDate =
    (c.field === "release" || c.field === "played" || c.field === "added") &&
    c.op !== "within" &&
    c.op !== "older" &&
    c.op !== "known" &&
    c.op !== "unknown";
  return {
    c,
    hasNumLow,
    numLow,
    hasNumHigh,
    numHigh,
    dayLow: needsDate ? calendarDay(c.value) : undefined,
    dayHigh: needsDate ? calendarDay(c.upper) : undefined,
  };
}
function matches(f: Facts, compiled: CompiledCondition, now: number): boolean {
  const { c } = compiled;
  if (c.field === "tags") {
    const gameTags = f.tags!;
    if (c.op === "known") return gameTags.size > 0;
    if (c.op === "unknown") return !gameTags.size;
    const tags = compiled.tags ?? [];
    if (!tags.length) return false;
    return c.op === "hasAny"
      ? tags.some((t) => gameTags.has(t))
      : c.op === "hasAll"
        ? tags.every((t) => gameTags.has(t))
        : c.op === "hasNone"
          ? tags.every((t) => !gameTags.has(t))
          : false;
  }
  const value = f[c.field];
  if (c.op === "unknown") return value === undefined;
  if (c.op === "known") return value !== undefined;
  if (value === undefined) return false;
  if (typeof value === "number") {
    const low = compiled.numLow!,
      high = compiled.numHigh!;
    if (!compiled.hasNumLow || !Number.isFinite(low)) return false;
    return c.op === "lt"
      ? value < low
      : c.op === "lte"
        ? value <= low
        : c.op === "gt"
          ? value > low
          : c.op === "gte"
            ? value >= low
            : c.op === "between" &&
              !!compiled.hasNumHigh &&
              Number.isFinite(high) &&
              value >= low &&
              value <= high;
  }
  const [start, end] = value;
  if (c.op === "within" || c.op === "older") {
    const days = compiled.numLow!;
    if (!compiled.hasNumLow || !Number.isFinite(days) || days < 0) return false;
    return c.op === "within"
      ? start >= now - days * DAY
      : end < now - days * DAY;
  }
  const low = compiled.dayLow,
    high = compiled.dayHigh;
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

function tagNormalizer() {
  // Reuse normalization for repeating labels within one library snapshot.
  const keys = new Map<string, string>();
  return (raw: string) => {
    let key = keys.get(raw);
    if (key === undefined) {
      key = tagKey(raw);
      keys.set(raw, key);
    }
    return key;
  };
}

function compileSmartGroups(rules: SmartGroup[], now: number) {
  const needed = new Set<RuleField>();
  const compiled = rules.map((rule) => ({
    match: rule.match,
    conditions: rule.conditions.map((condition) => {
      needed.add(condition.field);
      return compileCondition(condition);
    }),
  }));
  return {
    needed,
    indexOf: (f: Facts) =>
      compiled.findIndex(
        ({ match, conditions }) =>
          conditions.length &&
          (match === "all"
            ? conditions.every((c) => matches(f, c, now))
            : match === "any" && conditions.some((c) => matches(f, c, now))),
      ),
  };
}

function smartGroupIndex(
  rules: SmartGroup[],
  now: number,
  normalizedTag: (raw: string) => string,
) {
  const { needed, indexOf } = compileSmartGroups(rules, now);
  return (game: Game) => indexOf(facts(game, now, needed, normalizedTag));
}

// The editor creates a new preview builder whenever the library snapshot or
// clock changes. Keep only the most recent set of fields while editing rules.
export function createSmartGroupPreview(games: Game[], now: number) {
  const normalize = tagNormalizer();
  let cachedFields = "";
  let cachedFacts: Facts[] = [];
  return (rules: SmartGroup[], targetId: string, sampleLimit: number) => {
    const sample: Game[] = [];
    const target = rules.findIndex((rule) => rule.id === targetId);
    if (target < 0) return { count: 0, sample };

    // Earlier rules take precedence. Later rules cannot affect this collection.
    const { needed, indexOf } = compileSmartGroups(
      rules.slice(0, target + 1),
      now,
    );
    const fields = [...needed].sort().join(",");
    if (cachedFields !== fields) {
      cachedFields = fields;
      cachedFacts = [];
    }
    let count = 0;
    for (let i = 0; i < games.length; i++) {
      const game = games[i];
      const f = (cachedFacts[i] ??= facts(game, now, needed, normalize));
      if (indexOf(f) !== target) continue;
      count++;
      if (sample.length < sampleLimit) sample.push(game);
    }
    return { count, sample };
  };
}

function measurementBuckets(
  mode: "hltb" | "size",
  limits: number[],
  unit: string,
) {
  const names = [
    `Under ${limits[0]} ${unit}`,
    ...limits.slice(1).map((limit, i) => `${limits[i]}–${limit} ${unit}`),
    `${limits[limits.length - 1]}+ ${unit}`,
    mode === "hltb" ? "No HLTB time" : "Size unknown",
  ];
  return {
    limits,
    buckets: names.map((name, i) => ({ id: `${mode}:${i}`, name })),
  };
}

const measurementGroups = {
  hltb: measurementBuckets("hltb", [10, 25, 50, 100], "h"),
  size: measurementBuckets("size", [1, 10, 50, 100], "GiB"),
};

function visitGroups(
  games: Game[],
  mode: GroupMode,
  rules: SmartGroup[],
  now: number,
  add: (id: string, name: string, game: Game, index: number) => void,
) {
  if (mode === "none") return;
  const normalizedTag = tagNormalizer();
  const indexOfSmartGroup =
    mode === "smart" ? smartGroupIndex(rules, now, normalizedTag) : undefined;
  const smartIds = indexOfSmartGroup
    ? rules.map((rule) => `smart:${rule.id}`)
    : [];
  for (const game of games) {
    if (indexOfSmartGroup) {
      const index = indexOfSmartGroup(game);
      add(
        index < 0 ? "other" : smartIds[index],
        index < 0 ? "Other" : rules[index].name,
        game,
        index < 0 ? rules.length : index,
      );
      continue;
    }
    if (mode === "tags") {
      const tag =
        game.genres.find(normalizedTag) ?? game.tags?.find(normalizedTag);
      add(
        `tag:${tag ? normalizedTag(tag) : "unknown"}`,
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
      const { limits, buckets } = measurementGroups[mode];
      let index = 0;
      if (value === undefined) index = limits.length + 1;
      else while (index < limits.length && value >= limits[index]) index++;
      const { id, name } = buckets[index];
      add(id, name, game, index);
      continue;
    }
    const date = mode === "played" ? recentSortAt(game) : game.addedAt;
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
}

export function collectGroupMemberIds(
  games: Game[],
  mode: GroupMode,
  rules: SmartGroup[],
  now: number,
  selectedGroups: ReadonlyMap<string, string[]>,
) {
  visitGroups(games, mode, rules, now, (id, _name, game) => {
    selectedGroups.get(id)?.push(game.id);
  });
}

export function groupGames(
  games: Game[],
  mode: GroupMode,
  rules: SmartGroup[] = [],
  now = Date.now(),
): LibraryGroup[] {
  const groups = new Map<string, LibraryGroup>(),
    order = new Map<string, number>();
  visitGroups(games, mode, rules, now, (id, name, game, index) => {
    let group = groups.get(id);
    if (!group) {
      group = { id, name, games: [] };
      groups.set(id, group);
      order.set(id, index);
    }
    group.games.push(game);
  });
  return [...groups.values()].sort(
    (a, b) =>
      order.get(a.id)! - order.get(b.id)! || a.name.localeCompare(b.name),
  );
}
