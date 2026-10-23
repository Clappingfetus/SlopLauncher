<script lang="ts">
  import { Check, Image, LoaderCircle } from "@lucide/svelte";
  import Cover from "./Cover.svelte";
  import { untrack } from "svelte";
  import { currentResult } from "../lib/enrichment";
  import { isEmulated, selectedArtwork } from "../lib/artwork";
  import type { Game, ArtworkAsset, ArtworkChoice } from "../lib/types";
  let {
    game,
    revision,
    gridConfigured,
    onselect,
    initialSlot = "cover",
  }: {
    game: Game;
    initialSlot?: "cover" | "hero";
    revision: number;
    gridConfigured: boolean;
    onselect: (
      slot: "cover" | "hero",
      asset: (ArtworkAsset & { source: ArtworkChoice["source"] }) | null,
    ) => void;
  } = $props();
  let slot = $state<"cover" | "hero">(untrack(() => initialSlot));
  let grid = $derived(currentResult(game, "steamgriddb", revision));
  let chosen = $derived(selectedArtwork(game, slot));
  let options = $derived.by(() => {
    const choices: (ArtworkAsset & {
      source: ArtworkChoice["source"];
      label: string;
    })[] = [];
    if (game[slot])
      choices.push({
        url: game[slot]!,
        source: "store",
        label: isEmulated(game)
          ? "Original artwork"
          : "Original / store artwork",
      });
    for (const source of ["screenscraper", "igdb", "steamgriddb"] as const) {
      const data = currentResult(game, source, revision)?.data;
      const label =
        source === "steamgriddb"
          ? "SteamGridDB"
          : source === "igdb"
            ? "IGDB"
            : "ScreenScraper";
      const assets =
        source === "steamgriddb"
          ? slot === "cover"
            ? data?.covers
            : data?.heroes
          : undefined;
      for (const asset of assets?.length
        ? assets
        : data?.[slot]
          ? [
              {
                url: data[slot]!,
                author: data.author,
                page: data.artworkPage ?? data.url,
              },
            ]
          : []) {
        if (!choices.some((c) => c.url === asset.url))
          choices.push({ ...asset, source, label });
      }
    }
    // Keep a deliberate choice visible even after provider credentials/cache change.
    if (chosen && !choices.some((c) => c.url === chosen!.url))
      choices.unshift({ ...chosen, label: "Your selection" });
    return choices;
  });
</script>

<div class="artwork-picker">
  <p class="muted">{game.title}</p>
  <p class="artwork-policy">
    {isEmulated(game)
      ? "SteamGridDB never changes emulator artwork automatically. Check the game and platform before choosing an image."
      : "Store artwork is the default. SteamGridDB only fills missing covers. Your selections stay in place until you reset them."}
  </p>
  <div class="artwork-tabs" role="group" aria-label="Artwork type">
    <button
      class:active={slot === "cover"}
      aria-pressed={slot === "cover"}
      onclick={() => (slot = "cover")}>Box art</button
    >
    <button
      class:active={slot === "hero"}
      aria-pressed={slot === "hero"}
      onclick={() => (slot = "hero")}>Background</button
    >
  </div>
  <button
    class="artwork-default"
    aria-pressed={!chosen}
    onclick={() => onselect(slot, null)}
  >
    <span
      ><strong>Use default artwork</strong><small
        >{isEmulated(game)
          ? "Original artwork and ScreenScraper"
          : "Store first · fill gaps automatically"}</small
      ></span
    >
    {#if !chosen}<Check size={18} />{/if}
  </button>
  {#if !gridConfigured}<p class="muted">
      Connect SteamGridDB in Settings to browse more artwork.
    </p>
  {:else if !grid}<p class="artwork-status" role="status">
      <LoaderCircle class="spin" size={16} />Loading SteamGridDB artwork…
    </p>
  {:else if grid.status !== "ready"}<p class="muted" role="status">
      {grid.message ?? "No SteamGridDB artwork is available for this match."}
      {grid.status === "unavailable"
        ? "Your current artwork is kept; a retry is scheduled."
        : ""}
    </p>{/if}
  {#if options.length}
    <div class="artwork-options" class:backgrounds={slot === "hero"}>
      {#each options as option (option.url)}
        <button
          class="artwork-option"
          class:chosen={chosen?.url === option.url}
          aria-pressed={chosen?.url === option.url}
          aria-label={`Use ${option.label} ${slot === "cover" ? "box art" : "background"}${option.author ? ` by ${option.author}` : ""}`}
          onclick={() => onselect(slot, option)}
        >
          <span class="artwork-thumbnail"
            ><Cover
              game={{ ...game, cover: option.url, hero: option.url }}
              hero={slot === "hero"}
              thumbnail={slot === "hero" ? 512 : 256}
            /></span
          >
          <span class="artwork-caption"
            ><span>{option.label}<small>{option.author ?? ""}</small></span
            >{#if chosen?.url === option.url}<Check size={16} />{/if}</span
          >
        </button>
      {/each}
    </div>
  {:else}<div class="artwork-empty">
      <Image size={28} />
      <p>No {slot === "cover" ? "box art" : "backgrounds"} available yet.</p>
      <small
        >Check the provider match in Game Details if this game was
        misidentified.</small
      >
    </div>{/if}
</div>

<style>
  .artwork-picker {
    padding: 0 0 4px;
  }
  .artwork-policy {
    font-size: 13px;
    line-height: 1.6;
    color: var(--muted);
    margin: 12px 0 20px;
  }
  .artwork-tabs {
    display: flex;
    gap: 6px;
    margin-bottom: 16px;
  }
  .artwork-tabs button {
    padding: 9px 15px;
    border: 1px solid var(--line);
    border-radius: 7px;
  }
  .artwork-tabs .active {
    background: var(--surface-hover);
    color: var(--accent);
  }
  .artwork-default {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    padding: 14px;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: 9px;
    margin-bottom: 18px;
  }
  .artwork-default[aria-pressed="true"] {
    border-color: var(--accent);
  }
  small {
    display: block;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.5;
    margin-top: 4px;
  }
  .artwork-status {
    display: flex;
    gap: 8px;
    align-items: center;
    color: var(--muted);
    font-size: 13px;
  }
  .artwork-options {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
    gap: 14px;
  }
  .artwork-options.backgrounds {
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  }
  .artwork-option {
    display: block;
    min-width: 0;
    padding: 0;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: 9px;
    overflow: hidden;
  }
  .artwork-option.chosen {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }
  .artwork-thumbnail {
    display: block;
    aspect-ratio: 2/3;
    background: var(--surface);
    overflow: hidden;
  }
  .backgrounds .artwork-thumbnail {
    aspect-ratio: 16/7;
  }
  .artwork-thumbnail :global(img) {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }
  .artwork-caption {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 10px;
    font-size: 12px;
  }
  .artwork-caption > span {
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .artwork-empty {
    text-align: center;
    padding: 28px 12px;
    color: var(--muted);
  }
</style>
