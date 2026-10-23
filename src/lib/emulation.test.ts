import { describe, expect, it } from "vitest";
import {
  consoles,
  presetsFor,
  emulatorPresets,
  emulatedPlatform,
} from "./emulation";
import { demoGames } from "./demo";
import { identity, resourceLinks } from "./enrichment";
import { filterGames, mergeDiscovered } from "./library";

describe("emulated games in a changing library", () => {
  it("provides direct-launch presets for every requested console and keeps RetroArch away from unsupported systems", () => {
    for (const console of consoles)
      expect(presetsFor(console.id).some((p) => p.id !== "custom")).toBe(true);
    for (const console of ["switch", "ps3", "ps4", "wiiu"] as const)
      expect(presetsFor(console).some((p) => p.id === "retroarch")).toBe(false);
    for (const preset of emulatorPresets)
      expect(preset.args).toContain("{game}");
    expect(emulatorPresets.find((p) => p.id === "retroarch")?.args).toEqual([
      "-L",
      "{core}",
      "{game}",
    ]);
  });
  it("includes late additions in source filters, preserves launch data on rescans and binds enrichment to the console and game file", () => {
    const game = demoGames()[0];
    game.metadataId = undefined;
    game.copies = [
      {
        id: "emulated:new",
        source: "emulated",
        sourceId: "new",
        installed: true,
        emulation: {
          platform: "ps1",
          profileId: "profile",
          gamePath: "D:\\Games\\Title.chd",
        },
      },
    ];
    const library = mergeDiscovered(demoGames(), [game]);
    expect(
      filterGames(library, "emulated", "", false, "", "title"),
    ).toHaveLength(1);
    expect(emulatedPlatform(game)).toBe("ps1");
    expect(resourceLinks(game).store).toBeUndefined();
    expect(resourceLinks(game).discussions).toBeUndefined();
    const before = identity(game, "igdb");
    game.copies[0].emulation!.platform = "ps2";
    expect(identity(game, "igdb")).not.toBe(before);
    const consoleKey = identity(game, "igdb");
    game.copies[0].emulation!.gamePath = "D:\\Games\\Another.chd";
    expect(identity(game, "igdb")).not.toBe(consoleKey);
    const copy = mergeDiscovered([game], [])[0].copies[0];
    expect(copy.emulation?.profileId).toBe("profile");
    expect(game.favorite).toBe(true);
  });
});
