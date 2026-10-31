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
    <span class="item-icon"><Library size={15} /></span>
    <div class="item-body">
      <span class="item-label">Libraries</span><span
        class:working={libraryWorking}
        class="item-state">{libraryWorking ? "Syncing" : "Up to date"}</span
      >
    </div>
  </div>
  <div class="activity-item" title="Artwork download activity">
    <span class="item-icon"><Image size={15} /></span>
    <div class="item-body">
      <span class="item-label">Downloads</span><span
        class:working={downloadWorking}
        class="item-state">{downloadWorking ? "Working" : "Idle"}</span
      >
    </div>
  </div>
  <div class="activity-item progress-item">
    <span class="item-icon"><Database size={15} /></span>
    <button
      class="refresh"
      aria-label="Scan missing metadata"
      title="Scan games missing metadata"
      disabled={active || !onrefresh}
      onclick={onrefresh}><RotateCw size={14} /></button
    >
    <div class="item-body">
      <div class="item-top">
        <span class="item-label">Metadata</span><span class="item-state"
          >{active
            ? `${progress.completed} / ${progress.total}`
            : enabled
              ? "Ready"
              : "Paused"}</span
        >
      </div>
      <progress
        aria-label="Metadata and resource checks"
        max={Math.max(progress.total, 1)}
        value={active ? progress.completed : 0}
        class:quiet={!active}
      ></progress>
    </div>
  </div>
  <div class="activity-item progress-item cache-item">
    <span class="item-icon"><Layers size={15} /></span>
    <div class="item-body">
      <div class="item-top">
        <span class="item-label">Cover cache</span><span class="item-state"
          >{!catalog.total
            ? "Ready"
            : cacheActive
              ? `${settled} / ${catalog.total}`
              : "Complete"}</span
        >
      </div>
      <progress
        aria-label="Cover cache preparation"
        max={Math.max(catalog.total, 1)}
        value={settled}
      ></progress>
    </div>
  </div>
</section>

<style>
  .activity-bar {
    flex-shrink: 0;
    display: grid;
    grid-template-columns:
      minmax(115px, 0.85fr) minmax(115px, 0.85fr) minmax(190px, 1.4fr)
      minmax(190px, 1.4fr);
    gap: 0;
    padding: 10px 24px;
    border-top: 1px solid var(--line);
    background: var(--sidebar);
    color: var(--muted);
  }
  .activity-item {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 0 20px;
    border-right: 1px solid var(--line);
  }
  .activity-item:first-child {
    padding-left: 0;
  }
  .activity-item:last-child {
    padding-right: 0;
    border: 0;
  }
  .item-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border: 1px solid var(--line);
    border-radius: 9px;
    color: var(--accent);
    background: var(--surface);
  }
  .item-body {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .item-label {
    font-size: 11px;
    color: var(--text);
    font-weight: 600;
    white-space: nowrap;
  }
  .item-state {
    font-size: 10px;
    color: var(--faint);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .item-state.working {
    color: var(--accent);
  }
  .item-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .item-top .item-state {
    text-align: right;
    min-width: 9ch;
  }
  progress {
    width: 100%;
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
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    flex-shrink: 0;
    border: 1px solid var(--line);
    border-radius: 7px;
    color: var(--muted);
    background: transparent;
    cursor: pointer;
  }
  .refresh:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }
  .refresh:disabled {
    opacity: 0.4;
    cursor: default;
  }
  @media (max-width: 1200px) {
    .activity-bar {
      padding-inline: 16px;
    }
    .activity-item {
      padding-inline: 12px;
      gap: 8px;
    }
    .item-icon {
      width: 28px;
      height: 28px;
    }
    .item-top .item-state {
      min-width: 7ch;
    }
  }
  @media (max-width: 950px) {
    .activity-bar {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px 0;
    }
    .activity-item:nth-child(2) {
      border: 0;
    }
    .activity-item:nth-child(3) {
      padding-left: 0;
    }
  }
</style>
