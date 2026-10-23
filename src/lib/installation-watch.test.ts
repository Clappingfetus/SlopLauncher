import { expect, it } from "vitest";
import { InstallationWatch } from "./installation-watch";
import { demoGames } from "./demo";
import type { Game } from "./types";
const game = (): Game => ({
  ...demoGames()[0],
  id: "owned",
  copies: [
    { id: "epic:late", source: "epic", sourceId: "late", installed: false },
  ],
  notes: "Keep",
  favorite: true,
});
const evidence = (g: Game): Game => ({
  ...g,
  notes: "",
  copies: g.copies.map((c) => ({
    ...c,
    installed: true,
    installPath: "D:\\Games\\Late",
    executable: "D:\\Games\\Late\\game.exe",
    available: true,
  })),
});
it("detects late installations while retaining personal fields and unrelated games", () => {
  const watch = new InstallationWatch(),
    g = game(),
    other = { ...game(), id: "other", copies: [] };
  watch.start(g, g.copies[0], 1);
  const pending = watch.pending([g, other], 2);
  const updated = watch.apply([g, other], [evidence(g)], pending, 3);
  expect(updated[0].copies[0].installed).toBe(true);
  expect(updated[0].notes).toBe("Keep");
  expect(updated[0].favorite).toBe(true);
  expect(updated[1]).toBe(other);
  expect(watch.pending(updated, 4)).toEqual([]);
});
it("ignores removed, rematched and replaced install requests", () => {
  for (const change of ["removed", "identity", "path", "replaced"] as const) {
    const watch = new InstallationWatch(),
      g = game();
    watch.start(g, g.copies[0], 1);
    const pending = watch.pending([g], 2);
    const changed =
      change === "removed"
        ? []
        : [
            {
              ...g,
              copies: [
                {
                  ...g.copies[0],
                  ...(change === "identity"
                    ? { sourceId: "new" }
                    : change === "path"
                      ? { installPath: "E:\\Elsewhere" }
                      : {}),
                },
              ],
            },
          ];
    if (change === "replaced") watch.start(g, g.copies[0], 3);
    expect(watch.apply(changed, [evidence(g)], pending, 4)).toBe(changed);
  }
});
it("keeps absent or incomplete evidence usable, expires work and clears session state", () => {
  const watch = new InstallationWatch(),
    g = game(),
    games = [g];
  watch.start(g, g.copies[0], 1);
  const pending = watch.pending(games, 2);
  expect(watch.apply(games, [g], pending, 3)).toBe(games);
  expect(watch.pending(games, 2 * 60 * 60_000 + 1)).toEqual([]);
  watch.start(g, g.copies[0], 1);
  watch.clear();
  expect(watch.apply(games, [evidence(g)], pending, 3)).toBe(games);
});
