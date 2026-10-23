import type { Game } from "./types";
import type { ArtworkRequest } from "./artwork-loader";
import type { GridRow } from "./grid-layout";
export function artworkRowsWindow(
  rows: GridRow[],
  first: number,
  last: number,
  direction: number,
  size: number,
): ArtworkRequest[] {
  const requests: ArtworkRequest[] = [],
    seen = new Set<string>();
  const limit = Math.min(
    128,
    Math.floor((48 * 1024 * 1024) / (size * size * 1.5 * 4)),
  );
  const row = (index: number, priority: number) => {
    for (const game of rows[index]?.games ?? []) {
      const url = game.cover;
      if (!url || seen.has(url) || requests.length >= limit) continue;
      seen.add(url);
      requests.push({ url, thumbnail: size, priority });
    }
  };
  for (let i = first; i < last; i++) row(i, 0);
  row(first - 1, 1);
  row(last, 1);
  for (let distance = 1; distance <= 6; distance++) {
    row(
      direction >= 0 ? last + distance : first - 1 - distance,
      2 + distance / 10,
    );
    if (distance <= 2)
      row(
        direction >= 0 ? first - 1 - distance : last + distance,
        3 + distance / 10,
      );
  }
  return requests;
}
export function thumbnailWidth(cardWidth: number, dpr: number): number {
  const pixels = cardWidth * Math.min(2, Math.max(1, dpr));
  return pixels <= 256 ? 256 : pixels <= 384 ? 384 : 512;
}
export function artworkWindow(
  games: Game[],
  columns: number,
  first: number,
  last: number,
  direction: number,
  size: number,
): ArtworkRequest[] {
  const requests: ArtworkRequest[] = [];
  const seen = new Set<string>();
  const rows = Math.ceil(games.length / columns);
  const limit = Math.min(
    128,
    Math.floor((48 * 1024 * 1024) / (size * size * 1.5 * 4)),
  );
  const row = (index: number, priority: number) => {
    if (index < 0 || index >= rows) return;
    for (
      let i = index * columns;
      i < Math.min(games.length, (index + 1) * columns);
      i++
    ) {
      const url = games[i].cover;
      if (!url || seen.has(url) || requests.length >= limit) continue;
      seen.add(url);
      requests.push({ url, thumbnail: size, priority });
    }
  };
  for (let i = first; i < last; i++) row(i, 0);
  row(first - 1, 1);
  row(last, 1);
  for (let distance = 1; distance <= 6; distance++) {
    row(
      direction >= 0 ? last + distance : first - 1 - distance,
      2 + distance / 10,
    );
    if (distance <= 2)
      row(
        direction >= 0 ? first - 1 - distance : last + distance,
        3 + distance / 10,
      );
  }
  return requests;
}
