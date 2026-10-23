import { expect, it } from "vitest";
import { demoGames } from "./demo";
import { launchAction } from "./launch-action";
import type { Game } from "./types";
it("routes current Epic and GOG identities to native installers, preserving Steam installation", () => {
  const game: Game = {
    ...demoGames()[0],
    copies: [
      {
        id: "gog:1",
        source: "gog",
        sourceId: "1",
        installed: false,
        storePage: {
          source: "gog",
          sourceId: "1",
          url: "https://www.gog.com/en/game/example",
        },
      },
    ],
  };
  expect(launchAction(game)).toEqual({
    label: "Install with GOG Galaxy",
    unavailable: "",
  });
  game.copies[0].sourceId = "2";
  expect(launchAction(game).storeUrl).toBeUndefined();
  game.copies[0].source = "epic";
  expect(launchAction(game)).toEqual({
    label: "Install with Epic Games",
    unavailable: "",
  });
  game.copies = [
    { id: "steam:2", source: "steam", sourceId: "2", installed: false },
  ];
  expect(launchAction(game)).toEqual({
    label: "Install with Steam",
    unavailable: "",
  });
  game.copies[0].installed = true;
  expect(launchAction(game).label).toBe("Play now");
});
it("explains missing installs, missing copies and unavailable drives", () => {
  const game: Game = { ...demoGames()[0], copies: [] };
  expect(launchAction(game).unavailable).toBeTruthy();
  game.copies = [
    { id: "amazon:1", source: "amazon", sourceId: "1", installed: false },
  ];
  expect(launchAction(game).unavailable).toContain("Amazon Games");
  game.copies[0].installed = true;
  game.copies[0].available = false;
  expect(launchAction(game).label).toBe("Drive unavailable");
});

it("late owned imports refresh install identity without overwriting personal launch data", async () => {
  const { mergeOwnedLibrary } = await import("./library");
  const game: Game = {
    ...demoGames()[0],
    id: "epic:new",
    copies: [
      {
        id: "epic:new",
        source: "epic",
        sourceId: "new",
        ownership: { accountId: "fixture", verifiedAt: 1 },
        installed: false,
      },
    ],
    notes: "Keep me",
    favorite: true,
  };
  const incoming = {
    ...game,
    notes: "",
    favorite: false,
    copies: [
      {
        ...game.copies[0],
        launchUrl:
          "com.epicgames.launcher://apps/ns%3Acat%3Anew?action=launch&silent=true",
      },
    ],
  };
  const added = mergeOwnedLibrary([], [incoming], "epic");
  expect(launchAction(added[0]).label).toBe("Install with Epic Games");
  const merged = mergeOwnedLibrary([game], [incoming], "epic")[0];
  expect(merged.copies[0].launchUrl).toBe(incoming.copies[0].launchUrl);
  expect(merged.notes).toBe("Keep me");
  expect(merged.favorite).toBe(true);
});
