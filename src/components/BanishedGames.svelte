<script lang="ts">
  import { ChevronLeft, ChevronRight, Search, X } from "@lucide/svelte";
  import { searchText } from "../lib/search-index";
  import type { BanishedGame } from "../lib/banishment";
  import { onMount, tick } from "svelte";

  let {
    entries,
    busy = false,
    onrestore,
  }: {
    entries: BanishedGame[];
    busy?: boolean;
    onrestore: (ids: string[]) => Promise<void>;
  } = $props();

  const pageSize = 24;
  let query = $state("");
  let requestedPage = $state(0);
  let selected = $state.raw(new Set<string>());
  let list = $state<HTMLUListElement>();
  let searchInput: HTMLInputElement;
  let liveIds = $derived(new Set(entries.map((entry) => entry.id)));
  let searchable = $derived(
    entries.map((entry) => ({ entry, title: searchText(entry.title) })),
  );
  let needle = $derived(searchText(query).trim());
  let matches = $derived.by(() => {
    if (!needle) return entries;
    const matching: BanishedGame[] = [];
    for (const { entry, title } of searchable)
      if (title.includes(needle)) matching.push(entry);
    return matching;
  });
  let lastPage = $derived(Math.max(0, Math.ceil(matches.length / pageSize) - 1));
  let page = $derived(Math.min(requestedPage, lastPage));
  let start = $derived(page * pageSize);
  let shown = $derived(matches.slice(start, start + pageSize));
  let outsideSearch = $derived.by(() => {
    if (!needle || !selected.size) return 0;
    let count = selected.size;
    for (const entry of matches)
      if (selected.has(entry.id) && --count === 0) break;
    return count;
  });

  onMount(() => {
    void tick().then(() => searchInput?.focus());
  });

  $effect(() => {
    for (const id of selected) {
      if (!liveIds.has(id)) {
        selected = new Set([...selected].filter((id) => liveIds.has(id)));
        break;
      }
    }
  });

  $effect(() => {
    void matches;
    void start;
    if (list) list.scrollTop = 0;
  });

  function clearSearch() {
    query = "";
    requestedPage = 0;
    searchInput?.focus();
  }

  function changePage(step: number) {
    const next = page + step;
    if (next >= 0 && next <= lastPage) requestedPage = next;
  }

  function toggle(id: string, checked: boolean) {
    const next = new Set(selected);
    if (checked) next.add(id);
    else next.delete(id);
    selected = next;
  }

  async function restoreSelected() {
    await onrestore([...selected]);
    await tick();
    searchInput?.focus();
  }
</script>

<div class="banished-manager">
  <p class="field-hint">
    Banished games are blocked from library scans and imports. Unbanish a game to
    download fresh metadata and artwork.
  </p>
  <div class="library-search banished-search">
    <Search size={15} aria-hidden="true" />
    <input
      type="search"
      aria-label="Search banished games"
      placeholder="Search banished games…"
      autocomplete="off"
      bind:value={query}
      bind:this={searchInput}
      disabled={busy}
      oninput={() => (requestedPage = 0)}
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
        aria-label="Clear banished game search"
        title="Clear search (Esc)"
        disabled={busy}
        onclick={clearSearch}><X size={14} /></button
      >
    {/if}
  </div>

  <div class="banished-controls">
    <div class="selection-actions">
      <button
        class="text-button"
        disabled={busy || !matches.length}
        onclick={() => (selected = new Set(matches.map((entry) => entry.id)))}
        >{needle ? "Select all matches" : "Select all games"}</button
      >
      <button
        class="text-button"
        disabled={busy || !selected.size}
        onclick={() => {
          selected = new Set();
          searchInput?.focus();
        }}>Clear selection</button
      >
    </div>
    <div class="banished-pages">
      <span role="status">
        {#if lastPage}
          {(start + 1).toLocaleString()}–{(start + shown.length).toLocaleString()} of
          {matches.length.toLocaleString()}
        {:else}
          {matches.length} {matches.length === 1 ? "game" : "games"}
        {/if}
      </span>
      {#if lastPage}
        <button
          class="icon-button"
          aria-label="Previous page of banished games"
          aria-disabled={page === 0}
          disabled={busy}
          onclick={() => changePage(-1)}><ChevronLeft size={16} /></button
        >
        <button
          class="icon-button"
          aria-label="Next page of banished games"
          aria-disabled={page === lastPage}
          disabled={busy}
          onclick={() => changePage(1)}><ChevronRight size={16} /></button
        >
      {/if}
    </div>
  </div>

  {#if shown.length}
    <ul class="banished-list" aria-label="Banished games" bind:this={list}>
      {#each shown as entry (entry.id)}
        <li>
          <label class="banished-row">
            <input
              type="checkbox"
              disabled={busy}
              checked={selected.has(entry.id)}
              aria-label={`Restore ${entry.title}`}
              onchange={(event) => toggle(entry.id, event.currentTarget.checked)}
            />
            <span title={entry.title}>{entry.title}</span>
            <small>{new Date(entry.banishedAt).toLocaleDateString()}</small>
          </label>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="banished-empty field-hint">
      {entries.length ? "No matching games" : "No banished games"}
    </p>
  {/if}

  <div class="dialog-actions">
    <span role="status">
      {selected.size.toLocaleString()} selected
      {#if outsideSearch}<small>{outsideSearch.toLocaleString()} outside this search</small>{/if}
    </span>
    <button
      class="primary-button"
      disabled={busy || !selected.size}
      onclick={restoreSelected}
      >{busy ? "Restoring…" : "Unbanish selected"}</button
    >
  </div>
</div>

<style>
  .banished-search {
    margin-top: 16px;
    width: 100%;
  }
  .banished-search input::-webkit-search-cancel-button {
    display: none;
  }
  .banished-controls,
  .selection-actions,
  .banished-pages {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .banished-controls {
    flex-wrap: wrap;
    justify-content: space-between;
    margin: 8px 0;
    min-height: 32px;
  }
  .selection-actions {
    gap: 12px;
  }
  .banished-pages {
    color: var(--muted);
    font-size: 12px;
  }
  .banished-pages button[aria-disabled="true"] {
    color: var(--faint);
    cursor: default;
  }
  .banished-list {
    list-style: none;
    margin: 0;
    padding: 3px;
    max-height: min(360px, 44dvh);
    overflow-y: auto;
    border: 1px solid var(--line);
    border-radius: 7px;
  }
  .banished-empty {
    padding: 12px 4px;
  }
  .banished-row {
    min-height: 38px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px;
    border-radius: 4px;
    font-size: 13px;
    cursor: pointer;
  }
  .banished-row span {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .banished-row small {
    color: var(--muted);
    flex-shrink: 0;
  }
  .banished-row:hover,
  .banished-row:has(:checked) {
    background: var(--surface-hover);
  }
  .banished-row:has(:focus-visible) {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .dialog-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 14px;
  }
  .dialog-actions small {
    display: block;
    margin-top: 4px;
    color: var(--muted);
  }
</style>
