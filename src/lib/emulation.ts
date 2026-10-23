import type { Game } from "./types";
export const consoles = [
  { id: "ps1", name: "PlayStation", igdb: 7 },
  { id: "ps2", name: "PlayStation 2", igdb: 8 },
  { id: "ps3", name: "PlayStation 3", igdb: 9 },
  { id: "ps4", name: "PlayStation 4", igdb: 48 },
  { id: "switch", name: "Nintendo Switch", igdb: 130 },
  { id: "gamecube", name: "Nintendo GameCube", igdb: 21 },
  { id: "wiiu", name: "Nintendo Wii U", igdb: 41 },
  { id: "other", name: "Other consoles", igdb: undefined },
] as const;
export type ConsoleId = (typeof consoles)[number]["id"];
export const emulatorPresets = [
  {
    id: "duckstation",
    name: "DuckStation",
    consoles: ["ps1"],
    args: ["-batch", "--", "{game}"],
  },
  {
    id: "pcsx2",
    name: "PCSX2",
    consoles: ["ps2"],
    args: ["-batch", "--", "{game}"],
  },
  {
    id: "rpcs3",
    name: "RPCS3",
    consoles: ["ps3"],
    args: ["--no-gui", "{game}"],
  },
  { id: "shadps4", name: "shadPS4", consoles: ["ps4"], args: ["-g", "{game}"] },
  {
    id: "switch",
    name: "Switch · Eden / Yuzu-compatible",
    consoles: ["switch"],
    args: ["-g", "{game}"],
  },
  {
    id: "ryujinx",
    name: "Switch · Ryujinx-compatible",
    consoles: ["switch"],
    args: ["{game}"],
  },
  {
    id: "dolphin",
    name: "Dolphin",
    consoles: ["gamecube"],
    args: ["-b", "-e", "{game}"],
  },
  { id: "cemu", name: "Cemu", consoles: ["wiiu"], args: ["-g", "{game}"] },
  {
    id: "retroarch",
    name: "RetroArch",
    consoles: ["ps1", "ps2", "gamecube", "other"],
    args: ["-L", "{core}", "{game}"],
  },
  {
    id: "custom",
    name: "Other emulator",
    consoles: consoles.map((p) => p.id),
    args: ["{game}"],
  },
] as const;
export type EmulatorKind = (typeof emulatorPresets)[number]["id"];
export interface EmulatorProfile {
  id: string;
  name: string;
  platform: ConsoleId;
  kind: EmulatorKind;
  executable: string;
  corePath?: string;
  arguments: string[];
}
export interface EmulatedCopy {
  retroAchievementsGameId?: number;
  platform: ConsoleId;
  profileId: string;
  gamePath: string;
}
export interface EmulatedGameInput {
  title: string;
  profileId: string;
  gamePath: string;
}
export function consoleName(id?: string) {
  return consoles.find((p) => p.id === id)?.name ?? "Emulated";
}
export function presetsFor(platform: ConsoleId) {
  return emulatorPresets.filter((p) =>
    (p.consoles as readonly string[]).includes(platform),
  );
}
export function emulatedPlatform(game: Game) {
  const platforms = [
    ...new Set(
      game.copies.flatMap((c) => (c.emulation ? [c.emulation.platform] : [])),
    ),
  ];
  return platforms.length === 1 &&
    game.copies.every((c) => c.source === "emulated")
    ? platforms[0]
    : undefined;
}
