<script lang="ts">
  import { onDestroy } from "svelte";
  import { ActivityLatch } from "../lib/activity-latch";
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
  let libraryWorking = $state(false),
    downloadWorking = $state(false);
  const libraryActivity = new ActivityLatch(
    (value) => (libraryWorking = value),
  );
  const downloadActivity = new ActivityLatch(
    (value) => (downloadWorking = value),
  );
  $effect(() => libraryActivity.update(syncing.length > 0));
  $effect(() => downloadActivity.update(downloads > 0));
  onDestroy(() => {
    libraryActivity.dispose();
    downloadActivity.dispose();
  });
  let active = $derived(progress.running + progress.queued > 0);
  let metadataStatus = $derived(
    active
      ? `${progress.completed}/${progress.total}`
      : !enabled
        ? "Paused"
        : missing
          ? `${missing} gaps`
          : "Complete",
  );
  let metadataDetail = $derived(
    active
      ? `${progress.completed} of ${progress.total} metadata and resource checks; ${missing} games with gaps`
      : !enabled
        ? "Automatic artwork and metadata search is off"
        : missing
          ? `${missing} games have gaps; waiting for a provider, match, or retry`
          : "Artwork and metadata complete",
  );
</script>

<section class="enrichment-status" aria-label="Background activity">
  <div
    class="activity-lane"
    title={syncing.length
      ? `Syncing ${syncing.map((s) => sourceNames[s]).join(", ")}`
      : "No library sync running"}
  >
    <div class="lane-heading">
      <span>Libraries</span><span class="lane-value"
        >{libraryWorking ? "Syncing" : "Idle"}</span
      >
    </div>
    <div class="lane-detail">
      <span class:busy={libraryWorking} class="activity-dot"
      ></span>Owned-library sync
    </div>
  </div>
  <div
    class="activity-lane"
    title={`${downloads} artwork downloads currently active. Short requests are grouped into one activity session; cached images load from disk.`}
  >
    <div class="lane-heading">
      <span>Downloads</span><span class="lane-value"
        >{downloadWorking ? "Working" : "Idle"}</span
      >
    </div>
    <div class="lane-detail">
      <span class:busy={downloadWorking} class="activity-dot"></span>Artwork
    </div>
  </div>
  <div class="activity-lane" title={metadataDetail}>
    <div class="lane-heading">
      <span>Metadata</span><span class="lane-value">{metadataStatus}</span>
    </div>
    <div class="lane-detail">
      <progress
        class:inactive={!active}
        aria-label="Metadata and resource checks"
        aria-valuetext={metadataDetail}
        max={Math.max(1, progress.total)}
        value={active ? progress.completed : 0}
      ></progress>
    </div>
  </div>
  <div
    class="activity-lane cover-cache"
    title={catalog.total > catalog.prepared
      ? `Preparing persistent cover thumbnails: ${catalog.prepared} of ${catalog.total}. Visible artwork has priority.`
      : "Cover thumbnails are prepared on disk"}
  >
    <div class="lane-heading">
      <span>Cover cache</span><span class="lane-value"
        >{catalog.total ? `${catalog.prepared}/${catalog.total}` : "—"}</span
      >
    </div>
    <div class="lane-detail">
      <progress
        aria-label="Cover cache preparation"
        max={Math.max(1, catalog.total)}
        value={catalog.prepared}
      ></progress>
    </div>
  </div>
</section>

<style>
  /* Fixed tracks and permanent slots: activity changes never alter sibling positions.
     Counts align to the right using tabular digits; long values stay in their track. */
  .enrichment-status {
    flex-shrink: 0;
    display: grid;
    grid-template-columns:
      minmax(0, 0.9fr) minmax(0, 0.9fr) minmax(0, 1.2fr)
      minmax(0, 1.2fr);
    gap: 24px;
    padding: 8px 28px;
    border-top: 1px solid var(--line);
    background: var(--sidebar);
    color: var(--muted);
    font-size: 11px;
  }
  .activity-lane {
    min-width: 0;
  }
  .lane-heading {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 12ch;
    align-items: center;
    height: 15px;
    gap: 6px;
  }
  .lane-heading > span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .lane-value {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text);
  }
  .lane-detail {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 12px;
    margin-top: 4px;
    color: var(--faint);
    font-size: 10px;
    white-space: nowrap;
    overflow: hidden;
  }
  .activity-dot {
    width: 4px;
    height: 4px;
    border-radius: 50%;
    flex-shrink: 0;
    background: var(--line);
  }
  .activity-dot.busy {
    background: var(--accent);
  }
  progress {
    display: block;
    width: 100%;
    height: 3px;
    border: 0;
    border-radius: 3px;
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
  progress.inactive {
    opacity: 0.45;
  }
  @media (max-width: 1200px) {
    .enrichment-status {
      gap: 16px;
      padding-inline: 20px;
    }
    .lane-heading {
      grid-template-columns: minmax(0, 1fr) 10ch;
      font-size: 10px;
    }
  }
  @media (max-width: 950px) {
    .enrichment-status {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 10px 24px;
    }
  }
</style>
