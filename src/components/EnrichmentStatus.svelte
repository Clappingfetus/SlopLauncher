<script lang="ts">
  import { RotateCw, Library, Image, Database, Layers } from "@lucide/svelte";
  import { onDestroy } from "svelte";
  import { ActivityLatch } from "../lib/activity-latch";
  import type { AccountSource } from "../lib/types";
  import type { EnrichmentProgress } from "../lib/enrichment";
  let {
    progress,
    onrefresh,
    enabled,
    syncing = [],
    downloads = 0,
    catalog = { total: 0, prepared: 0, working: false },
  }: {
    progress: EnrichmentProgress;
    onrefresh?: () => void;
    enabled: boolean;
    syncing?: AccountSource[];
    downloads?: number;
    catalog?: {
      total: number;
      prepared: number;
      working: boolean;
      retrying?: number;
      pending?: number;
      nextRetryAt?: number;
    };
  } = $props();
  let libraryWorking = $state(false),
    downloadWorking = $state(false);
  const libraries = new ActivityLatch((v) => (libraryWorking = v)),
    artwork = new ActivityLatch((v) => (downloadWorking = v));
  $effect(() => libraries.update(syncing.length > 0));
  $effect(() => artwork.update(downloads > 0));
  onDestroy(() => {
    libraries.dispose();
    artwork.dispose();
  });
  let active = $derived(progress.running + progress.queued > 0);
  let settled = $derived(
    Math.min(catalog.total, catalog.prepared + (catalog.retrying ?? 0)),
  );
  let cacheActive = $derived((catalog.pending ?? 0) > 0);
</script>

<section class="activity-bar" aria-label="Background activity">
  <div class="activity-item" title="Connected library synchronization">
    <Library size={13} class="item-icon" />
    <span class="item-label">Libraries</span>
    <span class:working={libraryWorking} class="item-state"
      >{libraryWorking ? "Syncing" : "Up to date"}</span
    >
  </div>
  <div class="activity-item" title="Artwork download activity">
    <Image size={13} class="item-icon" />
    <span class="item-label">Downloads</span>
    <span class:working={downloadWorking} class="item-state"
      >{downloadWorking ? "Working" : "Idle"}</span
    >
  </div>
  <div class="activity-item progress-item">
    <Database size={13} class="item-icon" />
    <span class="item-label">Metadata</span>
    <button
      class="refresh"
      aria-label="Scan missing metadata"
      title="Scan games missing metadata"
      disabled={active || !onrefresh}
      onclick={onrefresh}><RotateCw size={12} /></button
    >
    <progress
      aria-label="Metadata and resource checks"
      max={Math.max(progress.total, 1)}
      value={active ? progress.completed : 0}
      class:quiet={!active}
    ></progress>
    <span class="item-state progress-state"
      >{active
        ? `${progress.completed} / ${progress.total}`
        : enabled
          ? "Ready"
          : "Paused"}</span
    >
  </div>
  <div class="activity-item progress-item cache-item">
    <Layers size={13} class="item-icon" />
    <span class="item-label">Cover cache</span>
    <progress
      aria-label="Cover cache preparation"
      max={Math.max(catalog.total, 1)}
      value={cacheActive ? settled : 0}
      class:quiet={!cacheActive}
    ></progress>
    <span class="item-state progress-state"
      >{!catalog.total
        ? "Ready"
        : cacheActive
          ? `${settled} / ${catalog.total}`
          : "Complete"}</span
    >
  </div>
</section>

<style>
  .activity-bar {
    flex-shrink: 0;
    display: grid;
    grid-template-columns: minmax(130px, 0.85fr) minmax(120px, 0.8fr) minmax(
        230px,
        1.6fr
      ) minmax(210px, 1.5fr);
    min-height: 34px;
    padding: 0 16px;
    border-top: 1px solid color-mix(in srgb, var(--line) 75%, transparent);
    background: var(--bg);
    color: var(--faint);
  }
  .activity-item {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 12px;
    border-right: 1px solid color-mix(in srgb, var(--line) 65%, transparent);
  }
  .activity-item:first-child {
    padding-left: 0;
  }
  .activity-item:last-child {
    padding-right: 0;
    border-right: 0;
  }
  .activity-item :global(.item-icon) {
    color: var(--faint);
    flex-shrink: 0;
  }
  .item-label,
  .item-state {
    white-space: nowrap;
    font-size: 10px;
    line-height: 1;
  }
  .item-label {
    color: var(--muted);
    font-weight: 600;
  }
  .item-state {
    color: var(--faint);
    font-variant-numeric: tabular-nums;
  }
  .item-state.working {
    color: var(--accent);
  }
  .activity-item:not(.progress-item) .item-state {
    margin-left: auto;
  }
  .progress-state {
    min-width: 8ch;
    text-align: right;
  }
  progress {
    min-width: 24px;
    width: 100%;
    flex: 1;
    height: 3px;
    border: 0;
    border-radius: 4px;
    overflow: hidden;
    background: var(--line);
    accent-color: var(--accent);
  }
  progress::-webkit-progress-bar {
    background: var(--line);
  }
  progress::-webkit-progress-value {
    background: var(--accent);
  }
  progress.quiet {
    opacity: 0.35;
  }
  .refresh {
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    margin-right: -4px;
    border-radius: 5px;
    color: var(--faint);
    background: transparent;
    cursor: pointer;
  }
  .refresh:hover:not(:disabled),
  .refresh:focus-visible {
    color: var(--accent);
    background: var(--surface-hover);
  }
  .refresh:disabled {
    opacity: 0.4;
    cursor: default;
  }
  @media (max-width: 1100px) {
    .activity-bar {
      padding-inline: 10px;
      grid-template-columns: minmax(105px, 0.85fr) minmax(100px, 0.8fr) minmax(
          195px,
          1.6fr
        ) minmax(180px, 1.5fr);
    }
    .activity-item {
      gap: 5px;
      padding-inline: 8px;
    }
    .progress-state {
      min-width: 7ch;
    }
  }
  @media (max-width: 820px) {
    .activity-bar {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      padding-block: 3px;
    }
    .activity-item {
      padding-block: 1px;
    }
    .activity-item:nth-child(2) {
      border-right: 0;
    }
    .activity-item:nth-child(3) {
      padding-left: 0;
    }
  }
</style>
