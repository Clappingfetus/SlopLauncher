<script lang="ts">
  import {
    ClipboardList,
    ImageOff,
    RefreshCw,
    ArrowUpRight,
    ChevronDown,
    ChevronLeft,
    ChevronRight,
    Search,
    X,
  } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import { missingFields } from "../lib/gaps";
  import { matchesSearch, searchText, searchWords } from "../lib/search-index";

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

  const pageSize = 24;
  const includeLocked = { includeLocked: true };
  let query = $state("");
  let searchInput: HTMLInputElement;
  let words = $derived(searchWords(query));
  let open = $state({ gaps: false, covers: false });
  let pages = $state({ gaps: 0, covers: 0 });
  const lists: Partial<Record<keyof typeof open, HTMLDivElement>> = {};

  let gaps = $derived.by(() => {
    const entries: { game: Game; fields: string[] }[] = [];
    for (const game of games) {
      const fields = missingFields(game, includeLocked);
      if (fields.length) entries.push({ game, fields });
    }
    return entries;
  });
  let failed = $derived.by(() => {
    const entries: { game: Game; retryAt: number }[] = [];
    if (!failures.length) return entries;
    const byUrl = new Map(
      failures.map((failure) => [failure.url, failure.retryAt]),
    );
    for (const game of games) {
      const retryAt = game.cover ? byUrl.get(game.cover) : undefined;
      if (retryAt !== undefined) entries.push({ game, retryAt });
    }
    return entries;
  });

  type Entry = (typeof gaps)[number] | (typeof failed)[number];
  const normalizedTitles = new WeakMap<Game, { title: string; text: string }>();

  function filterEntries<T extends Entry>(entries: T[], terms: string[]) {
    if (!terms.length) return entries;
    return entries.filter(({ game }) => {
      let cached = normalizedTitles.get(game);
      if (cached?.title !== game.title) {
        cached = { title: game.title, text: searchText(game.title) };
        normalizedTitles.set(game, cached);
      }
      return matchesSearch(cached.text, terms);
    });
  }

  let matchingGaps = $derived(filterEntries(gaps, words));
  let matchingCovers = $derived(filterEntries(failed, words));

  function resetPages() {
    setPage("gaps", 0);
    setPage("covers", 0);
  }

  function setPage(key: keyof typeof open, page: number) {
    pages[key] = page;
    const list = lists[key];
    if (list) list.scrollTop = 0;
  }

  function clearSearch() {
    query = "";
    resetPages();
    searchInput?.focus();
  }
</script>

{#snippet report(
  key: keyof typeof open,
  label: string,
  description: string,
  Icon: typeof ClipboardList,
  entries: Entry[],
  empty: string,
)}
  {@const lastPage = Math.max(0, Math.ceil(entries.length / pageSize) - 1)}
  {@const page = Math.min(pages[key], lastPage)}
  {@const start = page * pageSize}
  <details bind:open={open[key]}>
    <summary>
      <span class="health-icon"><Icon size={16} /></span>
      <span><strong>{label}</strong><small>{description}</small></span>
      <b>{entries.length.toLocaleString()}</b>
      <ChevronDown size={15} class="health-chevron" />
    </summary>
    {#if open[key]}
      {#if lastPage}
        <div class="health-pages">
          <span role="status"
            >{(start + 1).toLocaleString()}–{Math.min(
              start + pageSize,
              entries.length,
            ).toLocaleString()}
            of {entries.length.toLocaleString()}</span
          >
          <button
            class="icon-button"
            aria-label={`Previous page of ${label.toLowerCase()}`}
            aria-disabled={page === 0}
            onclick={() => {
              if (page > 0) setPage(key, page - 1);
            }}><ChevronLeft size={16} /></button
          >
          <button
            class="icon-button"
            aria-label={`Next page of ${label.toLowerCase()}`}
            aria-disabled={page === lastPage}
            onclick={() => {
              if (page < lastPage) setPage(key, page + 1);
            }}><ChevronRight size={16} /></button
          >
        </div>
      {/if}
      <div class="health-list" bind:this={lists[key]}>
        {#each entries.slice(start, start + pageSize) as item (item.game.id)}
          {@const detail =
            "fields" in item
              ? item.fields.join(" · ") +
                (item.game.metadataLocked ? " · Locked" : "")
              : item.retryAt > Date.now()
                ? `Retry after ${new Date(item.retryAt).toLocaleString()}`
                : "Retry pending"}
          {#if onopen}
            <button
              class="health-row health-row-button"
              onclick={() => onopen?.(item.game)}
            >
              <span
                ><strong>{item.game.title}</strong><small>{detail}</small></span
              >
              <ArrowUpRight size={13} />
            </button>
          {:else}
            <div class="health-row">
              <strong>{item.game.title}</strong><small>{detail}</small>
            </div>
          {/if}
        {:else}
          <p class="health-empty">
            {words.length ? "No matching games." : empty}
          </p>
        {/each}
      </div>
    {/if}
  </details>
{/snippet}

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
  <div class="library-search health-search">
    <Search size={15} aria-hidden="true" />
    <input
      type="search"
      aria-label="Search metadata and cover reports"
      placeholder="Search games in these reports…"
      autocomplete="off"
      bind:this={searchInput}
      bind:value={query}
      oninput={resetPages}
      onkeydown={(event) => {
        if (event.key !== "Escape" || event.isComposing || !query) return;
        event.preventDefault();
        event.stopPropagation();
        clearSearch();
      }}
    />
    {#if query}
      <button
        type="button"
        aria-label="Clear report search"
        onclick={clearSearch}><X size={14} /></button
      >
    {/if}
  </div>
  <div class="health-grid">
    {@render report(
      "gaps",
      "Missing metadata",
      "Games with incomplete details",
      ClipboardList,
      matchingGaps,
      "No missing metadata found.",
    )}
    {@render report(
      "covers",
      "Unavailable covers",
      "Failed cache attempts",
      ImageOff,
      matchingCovers,
      "No failed cover attempts.",
    )}
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
    align-items: start;
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
  .health-search {
    width: min(100%, 340px);
    margin-bottom: 12px;
  }
  .health-search input::-webkit-search-cancel-button {
    display: none;
  }
  .health-pages {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 5px;
    border-top: 1px solid var(--line);
    padding: 4px 8px;
  }
  .health-pages span {
    color: var(--muted);
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    margin-right: 4px;
  }
  .health-row-button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .health-pages button[aria-disabled="true"] {
    opacity: 0.45;
    cursor: default;
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
