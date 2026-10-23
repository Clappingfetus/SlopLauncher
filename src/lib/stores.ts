import type { Copy, Game, StorePage } from "./types";
import { preferredCopy } from "./library";

// Product URLs are supplied by the source, never made from a game's display title.
export function storePage(copy: Copy, page?: StorePage): string | undefined {
  if (!page || page.source !== copy.source || page.sourceId !== copy.sourceId)
    return undefined;
  try {
    const url = new URL(page.url);
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.port ||
      url.search ||
      url.hash
    )
      return undefined;
    const valid =
      copy.source === "epic"
        ? url.hostname === "store.epicgames.com" &&
          /^\/(?:[a-z]{2}-[A-Z]{2}\/)?p\/[a-zA-Z0-9_/-]+$/.test(url.pathname)
        : copy.source === "gog"
          ? ["www.gog.com", "gog.com"].includes(url.hostname) &&
            /^\/(?:[a-z]{2}\/)?game\/[a-zA-Z0-9_-]+\/?$/.test(url.pathname)
          : copy.source === "itch" &&
            /^[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.itch\.io$/.test(url.hostname) &&
            /^\/[a-zA-Z0-9_-]+\/?$/.test(url.pathname);
    return valid ? url.href : undefined;
  } catch {
    return undefined;
  }
}

export function storeLink(game: Game, pages: StorePage[] = []) {
  const copy = preferredCopy(game);
  if (!copy) return undefined;
  if (copy.source === "steam" && /^\d+$/.test(copy.sourceId)) {
    const id = Number(copy.sourceId);
    if (id > 0 && id <= 0xffffffff)
      return {
        name: "Steam Store",
        label: "Store",
        url: `https://store.steampowered.com/app/${id}/`,
      };
  }
  const name = { epic: "Epic Store", itch: "itch.io Store", gog: "GOG Store" }[
    copy.source as "epic" | "itch" | "gog"
  ];
  if (!name) return undefined;
  const canonical =
    storePage(copy, copy.storePage) ??
    pages.map((p) => storePage(copy, p)).find(Boolean);
  if (canonical) return { name, label: name, url: canonical };
  const query = encodeURIComponent(game.title);
  const url =
    copy.source === "epic"
      ? `https://store.epicgames.com/en-US/browse?q=${query}&sortBy=relevancy&sortDir=DESC&count=40`
      : copy.source === "gog"
        ? `https://www.gog.com/en/games?query=${query}`
        : `https://itch.io/search?q=${query}`;
  return {
    name: `Search ${name}`,
    label: `Search ${copy.source === "epic" ? "Epic" : copy.source === "gog" ? "GOG" : "itch.io"}`,
    url,
  };
}
