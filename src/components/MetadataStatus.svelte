<script lang="ts">
  import {
    ClipboardList,
    ImageOff,
    RefreshCw,
    ArrowUpRight,
    ChevronDown,
  } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import { missingFields } from "../lib/gaps";
  let {
    games,
    failures,
    onrefresh,
    onopen,
    busy = false,
  }: {
    games: Game[];
    failures: { url: string; retryAt: number }[];
    onrefresh: () => void;
    onopen?: (game: Game) => void;
    busy?: boolean;
  } = $props();
  let gapLimit = $state(60),
    failedLimit = $state(60);
  let gaps = $derived(
    games
      .map((game) => ({
        game,
        fields: missingFields({ ...game, metadataLocked: false }),
      }))
      .filter((x) => x.fields.length),
  );
  let failed = $derived.by(() => {
    const byUrl = new Map(failures.map((f) => [f.url, f]));
    return games.flatMap((game) => {
      const item = game.cover ? byUrl.get(game.cover) : undefined;
      return item ? [{ game, ...item }] : [];
    });
  });
</script>

<section class="health settings-panel" aria-label="Metadata and cache status">
  <div class="health-head">
    <div>
      <h2>Metadata & cover status</h2>
      <p>Review missing details and covers that could not be cached.</p>
    </div>
    <button class="secondary-button" disabled={busy} onclick={onrefresh}
      ><RefreshCw size={14} />Scan metadata</button
    >
  </div>
  <div class="health-grid">
    <details>
      <summary
        ><span class="health-icon"><ClipboardList size={16} /></span><span
          ><strong>Missing metadata</strong><small
            >Games with incomplete details</small
          ></span
        ><b>{gaps.length}</b><ChevronDown size={15} class="health-chevron" /></summary
      >
      <div class="health-list">
        {#each gaps.slice(0, gapLimit) as item (item.game.id)}{#if onopen}<button
              class="health-row health-row-button"
              onclick={() => onopen?.(item.game)}
              ><span
                ><strong>{item.game.title}</strong><small
                  >{item.fields.join(" · ")}{item.game.metadataLocked
                    ? " · Locked"
                    : ""}</small
                ></span
              ><ArrowUpRight size={13} /></button
            >{:else}<div class="health-row">
              <strong>{item.game.title}</strong><small
                >{item.fields.join(" · ")}{item.game.metadataLocked
                  ? " · Locked"
                  : ""}</small
              >
            </div>{/if}{:else}<p class="health-empty">
            No missing metadata found.
          </p>{/each}{#if gaps.length > gapLimit}<button
            class="text-button"
            onclick={() => (gapLimit += 60)}
            >Show more ({gapLimit} of {gaps.length})</button
          >{/if}
      </div>
    </details>
    <details>
      <summary
        ><span class="health-icon"><ImageOff size={16} /></span><span
          ><strong>Unavailable covers</strong><small
            >Failed cache attempts</small
          ></span
        ><b>{failed.length}</b><ChevronDown size={15} class="health-chevron" /></summary
      >
      <div class="health-list">
        {#each failed.slice(0, failedLimit) as item (item.game.id)}{#if onopen}<button
              class="health-row health-row-button"
              onclick={() => onopen?.(item.game)}
              ><span
                ><strong>{item.game.title}</strong><small
                  >{item.retryAt > Date.now()
                    ? `Retry after ${new Date(item.retryAt).toLocaleString()}`
                    : "Retry pending"}</small
                ></span
              ><ArrowUpRight size={13} /></button
            >{:else}<div class="health-row">
              <strong>{item.game.title}</strong><small
                >{item.retryAt > Date.now()
                  ? `Retry after ${new Date(item.retryAt).toLocaleString()}`
                  : "Retry pending"}</small
              >
            </div>{/if}{:else}<p class="health-empty">
            No failed cover attempts.
          </p>{/each}{#if failed.length > failedLimit}<button
            class="text-button"
            onclick={() => (failedLimit += 60)}
            >Show more ({failedLimit} of {failed.length})</button
          >{/if}
      </div>
    </details>
  </div>
</section>

<style>
  .health {
    margin-bottom: 20px;
  }
  .health-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 18px;
  }
  .health-head h2 {
    margin: 0 0 6px;
  }
  .health-head p {
    font-size: 12px;
    color: var(--muted);
    margin: 0;
    line-height: 1.5;
  }
  .health-head button {
    white-space: nowrap;
    font-size: 11px;
  }
  .health-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }
  .health details {
    border: 1px solid var(--line);
    border-radius: 9px;
    background: var(--bg);
    min-width: 0;
  }
  .health summary {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    list-style: none;
    padding: 13px;
  }
  .health summary::-webkit-details-marker {
    display: none;
  }
  .health summary > span:nth-child(2) {
    min-width: 0;
    flex: 1;
  }
  .health summary strong,
  .health summary small {
    display: block;
  }
  .health summary strong {
    font-size: 12px;
  }
  .health summary small {
    font-size: 10px;
    color: var(--muted);
    margin-top: 4px;
  }
  .health summary b {
    font-size: 13px;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }
  .health-icon {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: var(--surface);
    color: var(--accent);
  }
  .health-list {
    border-top: 1px solid var(--line);
    max-height: 320px;
    overflow: auto;
    padding: 0 13px 8px;
  }
  .health summary:hover {
    background: var(--surface);
    border-radius: 8px;
  }
  .health details[open] summary :global(.health-chevron) {
    transform: rotate(180deg);
  }
  .health summary :global(.health-chevron) {
    color: var(--faint);
    transition: transform 0.15s;
  }
  .health-row {
    padding: 10px 0;
    border-bottom: 1px solid var(--line);
  }
  .health-row-button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    width: 100%;
    text-align: left;
    margin-top: 0;
    padding: 10px 6px;
    border-radius: 5px;
    color: inherit;
  }
  .health-row-button > span {
    min-width: 0;
    flex: 1;
  }
  .health-row-button :global(svg) {
    color: var(--faint);
    flex-shrink: 0;
  }
  .health-row-button:hover {
    background: var(--surface);
  }
  .health-row-button:hover :global(svg) {
    color: var(--accent);
  }
  .health-row strong,
  .health-row small {
    display: block;
    overflow-wrap: anywhere;
  }
  .health-row strong {
    font-size: 11px;
    font-weight: 500;
  }
  .health-row small {
    font-size: 10px;
    color: var(--muted);
    line-height: 1.5;
    margin-top: 4px;
  }
  .health-empty {
    font-size: 11px;
    color: var(--muted);
    padding: 10px 0;
  }
  .health-list > .text-button {
    margin-top: 10px;
  }
  @media (max-width: 1100px) {
    .health-grid {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 700px) {
    .health-head {
      align-items: flex-start;
      flex-direction: column;
    }
  }
</style>
