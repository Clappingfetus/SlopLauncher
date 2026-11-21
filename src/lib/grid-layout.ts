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

const headerHeight = 52;

interface GridSection {
  games: Game[];
  group?: LibraryGroup;
  first: number;
  end: number;
  top: number;
  bottom: number;
}

function firstAfter(count: number, before: (index: number) => boolean) {
  let low = 0;
  let high = count;
  while (low < high) {
    const mid = (low + high) >>> 1;
    if (before(mid)) low = mid + 1;
    else high = mid;
  }
  return low;
}

export function gridLayout(
  games: Game[],
  groups: LibraryGroup[],
  columns: number,
  rowHeight: number,
) {
  // One section describes all rows in a group (or the entire ungrouped grid).
  const sections: GridSection[] = [];
  const groupOffsets = new Map<string, number>();
  let rowCount = 0;
  let height = 0;
  const append = (list: Game[], group?: LibraryGroup) => {
    if (!list.length) return;
    const first = rowCount;
    const top = height;
    if (group) groupOffsets.set(group.id, top);
    const gameRows = Math.ceil(list.length / columns);
    rowCount += gameRows + (group ? 1 : 0);
    height = top + (group ? headerHeight : 0) + gameRows * rowHeight;
    sections.push({
      games: list, group, first, end: rowCount, top, bottom: height,
    });
  };

  if (groups.length) {
    for (const group of groups) append(group.games, group);
  } else append(games);

  function row(index: number): GridRow | undefined {
    if (index < 0 || index >= rowCount) return;
    const section =
      sections[firstAfter(sections.length, (i) => sections[i].end <= index)];
    const { group, first, top } = section;
    if (group && index === first) {
      return {
        key: `header:${group.id}`,
        groupId: group.id,
        name: group.name,
        count: group.games.length,
        top,
        height: headerHeight,
      };
    }
    const position = index - first - (group ? 1 : 0);
    const start = position * columns;
    return {
      key: `${group?.id ?? "library"}:${start}`,
      top: top + (group ? headerHeight : 0) + position * rowHeight,
      height: rowHeight,
      games: section.games.slice(start, start + columns),
    };
  }

  function rowAt(offset: number): number {
    const section =
      sections[firstAfter(sections.length, (i) => sections[i].bottom <= offset)];
    if (!section) return Math.max(0, rowCount - 1);
    const headerRows = section.group ? 1 : 0;
    const gameTop = section.top + headerRows * headerHeight;
    if (offset < gameTop) return section.first;
    const firstGameRow = section.first + headerRows;
    return firstGameRow + firstAfter(section.end - firstGameRow, (i) =>
      gameTop + (i + 1) * rowHeight <= offset,
    );
  }

  return { rowCount, height, row, rowAt, groupOffsets };
}
