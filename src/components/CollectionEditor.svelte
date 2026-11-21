<script lang="ts">
  import { tick, untrack } from "svelte";
  import { ChevronLeft, ChevronRight, Search, Trash2, Folder, X } from "@lucide/svelte";
  import type { Game, GameCollection } from "../lib/types";
  import { collectionMembers } from "../lib/collections";
  import { narrowGameSearch, searchIndex, searchWords } from "../lib/search-index";
  import { sourceNames } from "../lib/types";
  let {
    initial,
    games,
    now = Date.now(),
    existing = false,
    onsave,
    ondelete,
    oncancel,
  }: {
    initial: GameCollection;
    games: Game[];
    now?: number;
    existing?: boolean;
    onsave: (value: GameCollection) => void;
    ondelete: () => void;
    oncancel: () => void;
  } = $props();
  const start = untrack(() => initial);
  const pageSize = 24;
  const compareTitles = new Intl.Collator().compare;
  let live = $state(!!start.rule);
  let automaticMembers = $derived(
    new Set(collectionMembers(start, games, now)),
  );
  let name = $state(start.name),
    members = $state(new Set(start.gameIds)),
    query = $state(""),
    requestedPage = $state(0),
    selectedOnly = $state(!!start.rule),
    error = $state(""),
    confirmDelete = $state(false);
  let searchInput = $state<HTMLInputElement>();
  let gameList = $state<HTMLDivElement>();
  let effectiveMembers = $derived(live ? automaticMembers : members);
  let sortedGames = $derived(
    [...games].sort((a, b) => compareTitles(a.title, b.title)),
  );
  // Index just the selected games when they fit on one page. Larger selections
  // can reuse the library search as users check and uncheck games.
  let smallSelection = $derived(selectedOnly && effectiveMembers.size <= pageSize);
  let searchableGames = $derived(
    smallSelection
      ? sortedGames.filter((game) => effectiveMembers.has(game.id))
      : sortedGames,
  );
  let words = $derived(searchWords(query));
  let search = $derived(
    narrowGameSearch(
      searchableGames.map((game) => ({ game, text: searchIndex(game).text })),
    ),
  );
  let foundGames = $derived(words.length ? search(words) : searchableGames);
  let matches = $derived(
    selectedOnly && !smallSelection
      ? foundGames.filter((game) => effectiveMembers.has(game.id))
      : foundGames,
  );
  let lastPage = $derived(Math.max(0, Math.ceil(matches.length / pageSize) - 1));
  let page = $derived(Math.min(requestedPage, lastPage));
  let offset = $derived(page * pageSize);
  let shown = $derived(matches.slice(offset, offset + pageSize));
  let allMatchesSelected = $derived(
    matches.length > 0 &&
      (selectedOnly || matches.every((game) => effectiveMembers.has(game.id))),
  );
  let count = $derived.by(() => {
    const selection = effectiveMembers;
    if (!selection.size) return 0;
    let total = 0;
    for (const game of games) {
      if (selection.has(game.id)) total++;
    }
    return total;
  });

  $effect(() => {
    void page;
    if (gameList) gameList.scrollTop = 0;
  });

  function resetList() {
    requestedPage = 0;
    if (gameList) gameList.scrollTop = 0;
  }
  function clearSearch() {
    query = "";
    resetList();
    searchInput?.focus();
  }
  function changePage(step: number) {
    const next = page + step;
    if (next >= 0 && next <= lastPage) requestedPage = next;
  }
  function toggle(id: string, input: HTMLInputElement) {
    const next = new Set(members);
    if (input.checked) next.add(id);
    else next.delete(id);
    members = next;
    if (selectedOnly) requestedPage = page;
    if (selectedOnly && !input.checked && document.activeElement === input) {
      const row = input.closest(".collection-game-row");
      const neighbor =
        row?.nextElementSibling?.querySelector<HTMLInputElement>("input") ??
        row?.previousElementSibling?.querySelector<HTMLInputElement>("input");
      void tick().then(() => {
        const target = neighbor?.isConnected
          ? neighbor
          : gameList?.querySelector<HTMLInputElement>("input:enabled") ?? searchInput;
        target?.focus();
      });
    }
  }
  function toggleMatches() {
    if (live || !matches.length) return;
    const next = new Set(members);
    if (allMatchesSelected) {
      for (const game of matches) next.delete(game.id);
      if (selectedOnly) {
        resetList();
        searchInput?.focus();
      }
    } else {
      for (const game of matches) next.add(game.id);
    }
    members = next;
  }
  function save(e: SubmitEvent) {
    e.preventDefault();
    error = "";
    try {
      onsave({
        id: start.id,
        name,
        gameIds: [...members],
        ...(live && start.rule ? { rule: start.rule } : {}),
      });
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    }
  }
</script>

<form class="collection-editor" onsubmit={save}>
  <label
    >Collection name<input
      aria-label="Collection name"
      bind:value={name}
      maxlength="80"
      required
      placeholder="e.g. Weekend games"
    /></label
  >
  {#if start.rule}<label class="collection-check"
      ><input
        type="checkbox"
        aria-label="Keep matching automatically"
        checked={live}
        onchange={(e) => {
          if (!e.currentTarget.checked)
            members = existing
              ? new Set(automaticMembers)
              : new Set(start.gameIds);
          live = e.currentTarget.checked;
          resetList();
        }}
      />Keep matching automatically</label
    >
    <p class="field-hint">
      {live
        ? "Live Collection: saved group rules match your whole library, including new games and metadata changes. Search and library filters are not saved. Membership follows the rules."
        : "Snapshot: current matches are saved as a regular Collection. You can add or remove games yourself."}
    </p>{/if}
  <div class="collection-selection-heading">
    <span role="status"><Folder size={15} />{count} {live ? "matching" : "selected"}</span
    ><label class="collection-check"
      ><input
        type="checkbox"
        bind:checked={selectedOnly}
        onchange={resetList}
      />Selected only</label
    >
  </div>
  <div class="library-search collection-search"
    ><Search size={16} /><input
      aria-label="Search games for collection"
      bind:this={searchInput}
      bind:value={query}
      oninput={resetList}
      onkeydown={(event) => {
        if (event.isComposing) return;
        if (event.key === "Enter") event.preventDefault();
        if (event.key === "Escape" && query) {
          event.preventDefault();
          clearSearch();
        }
      }}
      placeholder="Find games to include"
    />{#if query}<button
        type="button"
        aria-label="Clear collection search"
        title="Clear search (Esc)"
        onclick={clearSearch}><X size={14} /></button
      >{/if}</div
  >
  <div class="collection-results">
    <div class="collection-pages">
      <span role="status">
        {#if lastPage}
          {(offset + 1).toLocaleString()}–{(offset + shown.length).toLocaleString()} of
        {/if}
        {matches.length.toLocaleString()} {matches.length === 1 ? "match" : "matches"}
      </span>
      {#if lastPage}
        <button
          type="button"
          class="icon-button"
          aria-label="Previous game page"
          aria-disabled={page === 0}
          onclick={() => changePage(-1)}
        ><ChevronLeft size={16} /></button>
        <button
          type="button"
          class="icon-button"
          aria-label="Next game page"
          aria-disabled={page === lastPage}
          onclick={() => changePage(1)}
        ><ChevronRight size={16} /></button>
      {/if}
    </div>
    {#if !live}<button
        type="button"
        class="text-button"
        disabled={!matches.length}
        onclick={toggleMatches}
        >{allMatchesSelected ? "Deselect all matches" : "Select all matches"}</button
      >{/if}
  </div>
  <div
    class="collection-game-list"
    role="group"
    aria-label="Collection games"
    bind:this={gameList}
  >
    {#each shown as game (game.id)}
      <label class="collection-game-row"
        ><input
          type="checkbox"
          aria-label={`Include ${game.title}`}
          checked={effectiveMembers.has(game.id)}
          disabled={live}
          onchange={(e) => toggle(game.id, e.currentTarget)}
        /><span
          ><strong>{game.title}</strong><small
            >{[...new Set(game.copies.map((c) => sourceNames[c.source]))].join(
              " · ",
            )}</small
          ></span
        ></label
      >
    {:else}<p class="field-hint">
        {games.length
          ? "No games match this view."
          : "Your collection can start empty. Add games to your library and include them here later."}
      </p>
      {#if selectedOnly && games.length}<button
          type="button"
          class="text-button"
          onclick={() => {
            selectedOnly = false;
            resetList();
            searchInput?.focus();
          }}>Show all games</button
        >{/if}{/each}
  </div>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
  <div class="dialog-footer">
    <button type="button" class="secondary-button" onclick={oncancel}
      >Cancel</button
    ><button type="submit" class="primary-button"
      >{existing ? "Save collection" : "Create collection"}</button
    >
  </div>
</form>
{#if existing}<div class="collection-delete">
    {#if confirmDelete}<p>
        Delete this collection? Your games and files stay in your library.
      </p>
      <div class="provider-setting-actions">
        <button class="text-button" onclick={() => (confirmDelete = false)}
          >Keep collection</button
        ><button class="danger-button" onclick={ondelete}
          >Delete collection</button
        >
      </div>
    {:else}<button class="text-button" onclick={() => (confirmDelete = true)}
        ><Trash2 size={14} />Delete collection…</button
      >{/if}
  </div>{/if}
