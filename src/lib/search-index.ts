import type { Game } from "./types";
import { tagKey } from "./tags";

export function searchText(text: string) {
  return text
    .normalize("NFKD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    .replace(/[’']/g, "")
    .replace(/[‐‑–—:_-]/g, " ");
}

export function searchWords(query: string) {
  return searchText(query).trim().split(/\s+/).filter(Boolean);
}

export function matchesSearch(text: string, words: string[]) {
  for (const word of words) {
    if (!text.includes(word)) return false;
  }
  return true;
}

export function narrowGameSearch(entries: { game: Game; text: string }[]) {
  let previousWords: string[] = [];
  let previousMatches = entries;
  let previousGames: Game[] | undefined;

  return (words: string[]) => {
    // Narrow only if the new query still implies every previous requirement.
    const narrowing = previousWords.every((word) =>
      words.some((next) => next.includes(word)),
    );
    const candidates = narrowing ? previousMatches : entries;
    const required =
      narrowing && previousWords.length
        ? words.filter((word) => !previousWords.some((old) => old.includes(word)))
        : words;
    previousWords = words;

    // Keep the existing result array if all candidates still match. This also
    // avoids touching the library for edits that leave the requirements equal.
    let matches = candidates;
    if (required.length) {
      const firstMiss = candidates.findIndex(
        ({ text }) => !matchesSearch(text, required),
      );
      if (firstMiss !== -1) {
        matches = candidates.slice(0, firstMiss);
        for (let i = firstMiss + 1; i < candidates.length; i++) {
          const entry = candidates[i];
          if (matchesSearch(entry.text, required)) matches.push(entry);
        }
      }
    }
    if (!previousGames || matches !== previousMatches) {
      previousMatches = matches;
      previousGames = matches.map(({ game }) => game);
    }
    return previousGames;
  };
}

const cache = new WeakMap<
  Game,
  {
    title: string;
    sourceTags: string[];
    genres: string[];
    text: string;
    tags: Set<string>;
  }
>();
const same = (a: string[], b: string[]) =>
  a.length === b.length && a.every((value, i) => value === b[i]);
export function searchIndex(game: Game) {
  const sourceTags = game.tags ?? [];
  const old = cache.get(game);
  // Compare values without allocating a signature; snapshots catch in-place edits too.
  if (
    old &&
    old.title === game.title &&
    same(old.sourceTags, sourceTags) &&
    same(old.genres, game.genres)
  )
    return old;
  const tags = [...sourceTags, ...game.genres];
  const next = {
    title: game.title,
    sourceTags: [...sourceTags],
    genres: [...game.genres],
    text: searchText([game.title, ...tags].join(" ")),
    tags: new Set(tags.map(tagKey)),
  };
  cache.set(game, next);
  return next;
}
