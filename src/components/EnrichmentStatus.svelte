<script lang="ts">
  import {
    CheckCircle2,
    LoaderCircle,
    Pause,
    CircleHelp,
  } from "@lucide/svelte";
  import { sourceNames, type AccountSource } from "../lib/types";
  import type { EnrichmentProgress } from "../lib/enrichment";
  let {
    progress,
    enabled,
    missing,
    syncing = [],
    downloads = 0,
    catalog = { total: 0, prepared: 0, working: false },
  }: {
    progress: EnrichmentProgress;
    enabled: boolean;
    missing: number;
    syncing?: AccountSource[];
    downloads?: number;
    catalog?: { total: number; prepared: number; working: boolean };
  } = $props();
  const names: Record<string, string> = {
    achievements: "Steam achievements",
    retroachievements: "RetroAchievements",
    steam: "Steam",
    stores: "GOG",
    screenscraper: "ScreenScraper",
    igdb: "IGDB",
    steamgriddb: "SteamGridDB",
    pcgw: "PCGamingWiki",
    hltb: "HowLongToBeat",
    nexus: "Nexus Mods",
    "reviews-steam": "Steam ratings",
    "reviews-gog": "GOG ratings",
    "reviews-metacritic": "Metacritic users",
    "reviews-opencritic": "OpenCritic critics",
    "reviews-itch": "itch.io ratings",
  };
  let active = $derived(progress.running + progress.queued > 0);
  let providers = $derived([
    ...new Set(progress.active.map((j) => names[j.provider])),
  ]);
</script>

<section
  class="enrichment-status"
  class:working={active || syncing.length > 0 || downloads > 0}
  aria-label="Background activity"
>
  {#if syncing.length}<span class="activity-kind" role="status"
      ><LoaderCircle size={14} class="spin" />Library sync · {syncing
        .map((s) => sourceNames[s])
        .join(", ")}</span
    >{/if}
  {#if downloads > 0}<span class="activity-kind" role="status"
      >Artwork downloads · {downloads} active</span
    >{/if}
  {#if catalog.total > catalog.prepared}<span
      class="activity-kind"
      title="Prepares persistent thumbnails across your library without keeping all images in memory. Visible artwork has priority."
      >{catalog.working ? "Preparing covers" : "Cover cache"} · {catalog.prepared}/{catalog.total}</span
    >{/if}
  <div class="search-state" role="status" aria-live="polite">
    {#if active}<LoaderCircle size={14} class="spin" /><span
        >Metadata & resource checks <span class="request-count"
          >{progress.completed}/{progress.total} checks</span
        ></span
      >
    {:else if !enabled}<Pause size={13} /><span
        >Automatic artwork & details search off</span
      >
    {:else if !missing}<CheckCircle2 size={14} /><span
        >Artwork & details complete</span
      >
    {:else}<CircleHelp size={14} /><span
        >{missing}
        {missing === 1 ? "game has" : "games have"} gaps
        <span class="waiting">· waiting for a provider, match, or retry</span
        ></span
      >{/if}
  </div>
  {#if active}<progress
      aria-label="Metadata and resource checks"
      max={Math.max(1, progress.total)}
      value={progress.completed}
    ></progress>
    <span
      class="active-providers"
      title={progress.active
        .map((j) => `${names[j.provider]} · ${j.title}`)
        .join("\n")}>{providers.join(" · ") || "Queued"}</span
    >
    <span class="remaining">{missing} games with gaps</span>{/if}
</section>

<style>
  .activity-kind {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    font-size: 11px;
  }
  .enrichment-status {
    flex-shrink: 0;
    flex-wrap: wrap;
    min-height: 35px;
    display: flex;
    align-items: center;
    gap: 18px;
    border-top: 1px solid var(--line);
    padding: 8px 28px;
    background: var(--sidebar);
    color: var(--faint);
    font-size: 11px;
  }
  .enrichment-status.working {
    color: var(--muted);
  }
  .search-state {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .search-state :global(svg) {
    flex-shrink: 0;
    color: var(--accent);
  }
  .request-count {
    margin-left: 8px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  progress {
    width: 120px;
    min-width: 60px;
    height: 4px;
    border: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--line);
    accent-color: var(--accent);
  }
  progress::-webkit-progress-bar {
    background: var(--line);
    border-radius: 4px;
  }
  progress::-webkit-progress-value {
    background: var(--accent);
    border-radius: 4px;
    transition: width 0.2s;
  }
  .active-providers {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .remaining {
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }
  @media (max-width: 1100px) {
    .remaining,
    .waiting {
      display: none;
    }
    .enrichment-status {
      gap: 12px;
      padding-inline: 20px;
    }
  }
</style>
