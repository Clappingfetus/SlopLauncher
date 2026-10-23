import type { Game, LibraryGroup } from "./types";
export interface GridRow {
  key: string;
  top: number;
  height: number;
  games?: Game[];
  name?: string;
  count?: number;
  groupId?: string;
}
export function gridLayout(
  games: Game[],
  groups: LibraryGroup[],
  columns: number,
  rowHeight: number,
): { rows: GridRow[]; height: number } {
  const rows: GridRow[] = [];
  let top = 0;
  const append = (id: string, list: Game[]) => {
    for (let i = 0; i < list.length; i += columns) {
      rows.push({
        key: `${id}:${i}`,
        top,
        height: rowHeight,
        games: list.slice(i, i + columns),
      });
      top += rowHeight;
    }
  };
  if (!groups.length) append("library", games);
  else
    for (const group of groups) {
      if (!group.games.length) continue;
      rows.push({
        key: `header:${group.id}`,
        groupId: group.id,
        name: group.name,
        count: group.games.length,
        top,
        height: 52,
      });
      top += 52;
      append(group.id, group.games);
    }
  return { rows, height: top };
}
export function gridRowAt(rows: GridRow[], offset: number): number {
  let low = 0,
    high = rows.length;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (rows[mid].top + rows[mid].height <= offset) low = mid + 1;
    else high = mid;
  }
  return Math.min(Math.max(0, rows.length - 1), low);
}
