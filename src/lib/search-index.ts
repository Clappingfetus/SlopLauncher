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
