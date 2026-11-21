<script lang="ts">
  import { onMount, tick } from "svelte";
  import { Check, ChevronLeft, ChevronRight } from "@lucide/svelte";
  type Option = { key: string; label: string; searchKey?: string; count?: number };

  let {
    options,
    noun,
    normalize,
    selected,
    clearLabel,
    onselect,
  }: {
    options: readonly Option[];
    noun: "tag" | "group";
    normalize: (query: string) => string;
    selected?: string;
    clearLabel?: string;
    onselect: (key: string) => void;
  } = $props();

  const pageSize = 24;
  let query = $state("");
  let requestedPage = $state(0);
  let searchInput: HTMLInputElement;
  let list = $state<HTMLUListElement>();
  let needle = $derived(normalize(query));
  let matches = $derived(
    needle
      ? options.filter((option) => (option.searchKey ?? option.key).includes(needle))
      : options,
  );
  let lastPage = $derived(
    Math.max(0, Math.ceil(matches.length / pageSize) - 1),
  );
  let page = $derived(Math.min(requestedPage, lastPage));
  let start = $derived(page * pageSize);
  let shown = $derived(matches.slice(start, start + pageSize));

  onMount(() => {
    void tick().then(() => searchInput?.focus());
  });

  $effect(() => {
    void matches;
    void start;
    if (list) list.scrollTop = 0;
  });

  function changePage(step: number) {
    const next = page + step;
    if (next >= 0 && next <= lastPage) requestedPage = next;
  }
</script>

<form
  onsubmit={(event) => {
    event.preventDefault();
    if (matches.length === 1) onselect(matches[0].key);
  }}
>
  <input
    bind:this={searchInput}
    bind:value={query}
    oninput={() => (requestedPage = 0)}
    aria-label={`Search ${noun}s`}
    placeholder={`Search ${noun}s`}
    autocomplete="off"
  />
</form>

<div class="picker-controls">
  {#if clearLabel}
    <button type="button" class="secondary-button" onclick={() => onselect("")}
      >{clearLabel}</button
    >
  {/if}
  <div class="picker-pages">
    <span role="status">
      {#if lastPage}
        {(start + 1).toLocaleString()}–{(start + shown.length).toLocaleString()} of
        {matches.length.toLocaleString()}
      {:else}
        {matches.length} {noun}{matches.length === 1 ? "" : "s"}
      {/if}
    </span>
    {#if lastPage > 0}
      <button
        type="button"
        class="icon-button"
        aria-label={`Previous ${noun} page`}
        aria-disabled={page === 0}
        onclick={() => changePage(-1)}
      >
        <ChevronLeft size={16} />
      </button>
      <button
        type="button"
        class="icon-button"
        aria-label={`Next ${noun} page`}
        aria-disabled={page === lastPage}
        onclick={() => changePage(1)}
      >
        <ChevronRight size={16} />
      </button>
    {/if}
  </div>
</div>

{#if shown.length}
  <ul bind:this={list} aria-label={`Matching ${noun}s`}>
    {#each shown as option (option.key)}
      <li>
        <button
          type="button"
          aria-pressed={selected === undefined ? undefined : selected === option.key}
          onclick={() => onselect(option.key)}
        >
          <span>{option.label}</span>
          {#if option.count !== undefined}
            <span class="option-count">{option.count.toLocaleString()}</span>
          {:else if selected === option.key}<Check size={15} />{/if}
        </button>
      </li>
    {/each}
  </ul>
{:else}
  <p>{needle ? `No matching ${noun}s.` : `No ${noun}s in this view.`}</p>
{/if}

<style>
  input {
    width: 100%;
    padding: 9px 11px;
  }
  .picker-controls,
  .picker-pages {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .picker-controls {
    flex-wrap: wrap;
    justify-content: space-between;
    margin-block: 12px 8px;
  }
  .picker-pages span,
  p {
    color: var(--muted);
    font-size: 12px;
  }
  .picker-pages button[aria-disabled="true"] {
    color: var(--faint);
    cursor: default;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 3px;
    max-height: min(320px, 45dvh);
    overflow-y: auto;
    scrollbar-width: thin;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  li button {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    min-height: 32px;
    padding: 5px 8px;
    border-radius: 4px;
    font-size: 12px;
    text-align: left;
    overflow-wrap: anywhere;
  }
  li button:hover,
  li button[aria-pressed="true"] {
    background: var(--surface);
    color: var(--accent);
  }
  li button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .option-count {
    flex-shrink: 0;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  p {
    padding: 12px 0;
  }
</style>
