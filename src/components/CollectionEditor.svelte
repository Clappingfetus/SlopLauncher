<script lang="ts">
  import { untrack } from "svelte";
  import { Search, Trash2, Folder } from "@lucide/svelte";
  import type { Game, GameCollection } from "../lib/types";
  import { collectionMembers } from "../lib/collections";
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
  let live = $state(!!start.rule);
  let automaticMembers = $derived(
    new Set(collectionMembers(start, games, now)),
  );
  let name = $state(start.name),
    members = $state(new Set(start.gameIds)),
    query = $state(""),
    limit = $state(80),
    selectedOnly = $state(!!start.rule),
    error = $state(""),
    confirmDelete = $state(false);
  let effectiveMembers = $derived(live ? automaticMembers : members);
  let matches = $derived(
    games
      .filter(
        (g) =>
          (!selectedOnly || effectiveMembers.has(g.id)) &&
          g.title
            .normalize("NFKC")
            .toLowerCase()
            .includes(query.normalize("NFKC").trim().toLowerCase()),
      )
      .sort((a, b) => a.title.localeCompare(b.title)),
  );
  let count = $derived(games.filter((g) => effectiveMembers.has(g.id)).length);
  function toggle(id: string, checked: boolean) {
    const next = new Set(members);
    if (checked) next.add(id);
    else next.delete(id);
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
        }}
      />Keep matching automatically</label
    >
    <p class="field-hint">
      {live
        ? "Live Collection: saved group rules match your whole library, including new games and metadata changes. Search and library filters are not saved. Membership follows the rules."
        : "Snapshot: current matches are saved as a regular Collection. You can add or remove games yourself."}
    </p>{/if}
  <div class="collection-selection-heading">
    <span><Folder size={15} />{count} {live ? "matching" : "selected"}</span
    ><label class="collection-check"
      ><input
        type="checkbox"
        bind:checked={selectedOnly}
        onchange={() => (limit = 80)}
      />Selected only</label
    >
  </div>
  <label class="collection-search"
    ><Search size={16} /><input
      aria-label="Search games for collection"
      bind:value={query}
      oninput={() => (limit = 80)}
      placeholder="Find games to include"
    /></label
  >
  <div class="collection-game-list" role="group" aria-label="Collection games">
    {#each matches.slice(0, limit) as game (game.id)}
      <label class="collection-game-row"
        ><input
          type="checkbox"
          aria-label={`Include ${game.title}`}
          checked={effectiveMembers.has(game.id)}
          disabled={live}
          onchange={(e) => toggle(game.id, e.currentTarget.checked)}
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
      </p>{/each}
    {#if matches.length > limit}<button
        type="button"
        class="text-button"
        onclick={() => (limit += 80)}
        >Show more games ({limit} of {matches.length})</button
      >{/if}
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
