import { describe, it, expect } from "vitest";
import { addLibraryFolder, folderIdentity } from "./folders";
import { sources, type LibraryFolders } from "./types";

describe("source library folders", () => {
  it("keeps multiple folders per store and preserves other source lists", () => {
    const config: LibraryFolders = {
      steam: ["D:\\SteamLibrary"],
      itch: ["E:\\itch"],
    };
    const added = addLibraryFolder(config, "steam", "F:\\SteamLibrary");
    expect(added).toEqual(["D:\\SteamLibrary", "F:\\SteamLibrary"]);
    expect(config.steam).toEqual(["D:\\SteamLibrary"]);
    expect(config.itch).toEqual(["E:\\itch"]);
  });
  it("rejects Windows case, separator and trailing-slash duplicates without removing drive roots", () => {
    expect(folderIdentity("D:/Games/")).toBe(folderIdentity("d:\\games"));
    expect(() =>
      addLibraryFolder({ gog: ["D:\\Games"] }, "gog", "d:/games/"),
    ).toThrow("already included");
    expect(folderIdentity("C:\\")).not.toBe(folderIdentity("D:\\"));
  });
  it("adds future folders independently to every store", () => {
    for (const source of sources.filter(
      (s) => s !== "manual" && s !== "emulated",
    )) {
      const first = addLibraryFolder({}, source, "D:\\Games");
      expect(
        addLibraryFolder({ [source]: first }, source, "E:\\More Games"),
      ).toHaveLength(2);
    }
  });
});
