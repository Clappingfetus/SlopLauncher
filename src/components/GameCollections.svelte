<script lang="ts">
  import { ChevronLeft, ChevronRight, FolderPlus, Search, X } from "@lucide/svelte";
  import type { GameCollection } from "../lib/types";
  import { matchesSearch, searchText, searchWords } from "../lib/search-index";
  let {
    gameId,
    collections,
    disabled = false,
    onchange,
    oncreate,
  }: {
    gameId: string;
    collections: GameCollection[];
    disabled?: boolean;
    onchange: (id: string, included: boolean) => void;
    oncreate: () => void;
  } = $props();
  const pageSize = 24;
  let query = $state("");
  let requestedPage = $state(0);
  let searchInput = $state<HTMLInputElement>();
  let choices = $state<HTMLDivElement>();
  let words = $derived(searchWords(query));
  let matches = $derived(
    words.length
      ? collections.filter((c) => matchesSearch(searchText(c.name), words))
      : collections,
  );
  let lastPage = $derived(Math.max(0, Math.ceil(matches.length / pageSize) - 1));
  let page = $derived(Math.min(requestedPage, lastPage));
  let offset = $derived(page * pageSize);
  let shown = $derived(matches.slice(offset, offset + pageSize));
  $effect(() => {
    void gameId;
    query = "";
    requestedPage = 0;
  });
  $effect(() => {
    void page;
    if (choices) choices.scrollTop = 0;
  });
  function resetResults() {
    requestedPage = 0;
    if (choices) choices.scrollTop = 0;
  }
  function clearSearch() {
    searchInput?.focus({ preventScroll: true });
    query = "";
    resetResults();
  }
  function changePage(step: number) {
    const next = page + step;
    if (next >= 0 && next <= lastPage) requestedPage = next;
  }
</script>

<section class="game-collections" aria-label="Game collections">
  <h2>
    Collections<button class="text-button" {disabled} onclick={oncreate}
      ><FolderPlus size={14} />New collection</button
    >
  </h2>
  {#if disabled}<p class="field-hint">
      Collections are available in your own library.
    </p>
  {:else if collections.length}
    {#if collections.length > 8 || query}
      <div class="library-search game-collection-search">
        <Search size={15} aria-hidden="true" />
        <input
          type="search"
          aria-label="Find a collection"
          placeholder="Find a collection…"
          bind:this={searchInput}
          bind:value={query}
          oninput={resetResults}
          onkeydown={(event) => {
            if (event.key === "Escape" && !event.isComposing && query) {
              event.preventDefault();
              event.stopPropagation();
              clearSearch();
            }
          }}
        />
        {#if query}<button
            type="button"
            aria-label="Clear collection search"
            title="Clear search (Esc)"
            onclick={clearSearch}><X size={14} /></button
          >{/if}
      </div>
    {/if}
    {#if lastPage || words.length}
      <div class="collection-pages game-collection-pages">
        <span role="status">
          {#if !matches.length}
            No matching collections
          {:else if lastPage}
            {offset + 1}–{offset + shown.length} of {matches.length} collections
          {:else}
            {matches.length} of {collections.length} collections
          {/if}
        </span>
        {#if lastPage}
          <button
            type="button"
            class="icon-button"
            aria-label="Previous collection page"
            aria-disabled={page === 0}
            onclick={() => changePage(-1)}
          ><ChevronLeft size={16} /></button>
          <button
            type="button"
            class="icon-button"
            aria-label="Next collection page"
            aria-disabled={page === lastPage}
            onclick={() => changePage(1)}
          ><ChevronRight size={16} /></button>
        {/if}
      </div>
    {/if}
    {#if matches.length}<div class="game-collection-choices" bind:this={choices}>
      {#each shown as collection (collection.id)}<label
          class="collection-check"
          title={collection.rule ? "Membership follows saved group rules" : undefined}
          ><input
            type="checkbox"
            aria-label={`Add to ${collection.name}`}
            checked={collection.gameIds.includes(gameId)}
            disabled={!!collection.rule}
            onchange={(e) => onchange(collection.id, e.currentTarget.checked)}
          /><span>{collection.name}{collection.rule ? " · Live" : ""}</span
          ></label
        >{/each}
    </div>{/if}
  {:else}<p class="field-hint">
      Group games into your own folders. A game can belong to several
      collections.
    </p>{/if}
</section>
