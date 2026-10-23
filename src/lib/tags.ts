export function tagKey(value: string): string {
  return value
    .normalize("NFKC")
    .trim()
    .replace(/\s+/g, " ")
    .toLocaleLowerCase();
}
function collectTags(
  groups: Iterable<readonly string[] | undefined>,
  limit: number,
): string[] {
  const labels = new Map<string, string>();
  for (const group of groups) {
    for (const raw of group ?? []) {
      if (typeof raw !== "string") continue;
      const label = raw
        .normalize("NFKC")
        .replace(/[\u0000-\u001f\u007f]/g, " ")
        .trim()
        .replace(/\s+/g, " ");
      if (!label || label.length > 80) continue;
      if (!labels.has(tagKey(label))) labels.set(tagKey(label), label);
      if (labels.size >= limit) break;
    }
    if (labels.size >= limit) break;
  }
  return [...labels.values()].sort((a, b) => a.localeCompare(b));
}
export function mergeTags(
  ...groups: (readonly string[] | undefined)[]
): string[] {
  return collectTags(groups, 256);
}
export function libraryTags(
  games: { tags?: string[]; genres: string[] }[],
): string[] {
  function* groups() {
    for (const game of games) {
      yield game.tags;
      yield game.genres;
    }
  }
  return collectTags(groups(), Infinity);
}
