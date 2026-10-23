import type { Game } from "./types";
import { sourceNames } from "./types";
import { preferredCopy } from "./library";
import { storeLink } from "./stores";
import { currentResult } from "./enrichment-identity";

const installers: Partial<Record<string, string>> = {
  steam: "Steam",
  itch: "itch",
  epic: "Epic Games",
  gog: "GOG Galaxy",
};

export function launchAction(game: Game) {
  const copy = preferredCopy(game);
  if (!copy)
    return {
      label: "No game version",
      unavailable: "Add a launch copy in Game Details.",
    };
  if (copy.available === false)
    return {
      label: "Drive unavailable",
      unavailable: "Reconnect the game drive and refresh installed games.",
    };
  if (copy.installed)
    return {
      label: copy.source === "itch" ? "Play with itch" : "Play now",
      unavailable: "",
    };
  const installer = installers[copy.source];
  if (installer) return { label: `Install with ${installer}`, unavailable: "" };
  const store = storeLink(
    game,
    currentResult(game, "stores")?.data?.storePages,
  );
  if (store)
    return {
      label: `Open ${sourceNames[copy.source]}`,
      unavailable: "",
      storeUrl: store.url,
    };
  return {
    label: "Not installed",
    unavailable: `Install this game in ${sourceNames[copy.source] ?? "its source"} first, then refresh installed games.`,
  };
}
