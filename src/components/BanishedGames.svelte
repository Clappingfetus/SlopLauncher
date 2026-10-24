<script lang="ts">
  import type { BanishedGame } from "../lib/banishment";
  let {
    entries,
    busy = false,
    onrestore,
  }: {
    entries: BanishedGame[];
    busy?: boolean;
    onrestore: (ids: string[]) => void;
  } = $props();
  let query = $state(""),
    selected = $state.raw(new Set<string>());
  $effect(() => {
    const live = new Set(entries.map((b) => b.id));
    if ([...selected].some((id) => !live.has(id)))
      selected = new Set([...selected].filter((id) => live.has(id)));
  });
  let top = $state(0);
  let list = $state<HTMLDivElement>();
  let start = $derived(Math.max(0, Math.floor(top / 38) - 5));
  $effect(() => {
    void query;
    top = 0;
    if (list) list.scrollTop = 0;
  });
  let visible = $derived(
    entries.filter((b) => b.title.toLowerCase().includes(query.toLowerCase())),
  );
</script>

<div class="banished-manager">
  <p class="field-hint">
    These games are blocked from every library and scan. Restore them to
    download fresh metadata and artwork.
  </p>
  <div class="banished-controls">
    <input
      aria-label="Search banished games"
      placeholder="Search banished games…"
      bind:value={query}
    /><button
      class="text-button"
      disabled={busy}
      onclick={() => (selected = new Set(visible.map((b) => b.id)))}
      >Select all shown</button
    ><button class="text-button" onclick={() => (selected = new Set())}
      >Clear</button
    >
  </div>
  <div
    class="banished-list"
    bind:this={list}
    onscroll={(event) => (top = event.currentTarget.scrollTop)}
  >
    <div class="banished-space" style:height={`${visible.length * 38}px`}>
      {#each visible.slice(start, start + 32) as entry, index (entry.id)}<label
          class="banished-row"
          style:top={`${(start + index) * 38}px`}
          ><input
            type="checkbox"
            disabled={busy}
            checked={selected.has(entry.id)}
            aria-label={`Restore ${entry.title}`}
            onchange={(event) => {
              const next = new Set(selected);
              event.currentTarget.checked
                ? next.add(entry.id)
                : next.delete(entry.id);
              selected = next;
            }}
          /><span>{entry.title}</span><small
            >{new Date(entry.banishedAt).toLocaleDateString()}</small
          ></label
        >{:else}<p class="field-hint">
          {entries.length ? "No matching games" : "No banished games"}
        </p>{/each}
    </div>
  </div>
  <div class="dialog-actions">
    <span>{selected.size} selected</span><button
      class="primary-button"
      disabled={busy || !selected.size}
      onclick={() => onrestore([...selected])}
      >{busy ? "Restoring…" : "Unbanish selected"}</button
    >
  </div>
</div>

<style>
  .banished-controls {
    display: flex;
    gap: 12px;
    margin: 18px 0;
  }
  .banished-controls input {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 10px;
    flex: 1;
    min-width: 0;
  }
  .banished-list {
    height: min(52vh, 560px);
    overflow: auto;
    border-block: 1px solid var(--line);
  }
  .banished-space {
    position: relative;
    min-height: 38px;
  }
  .banished-row {
    position: absolute;
    left: 0;
    right: 0;
    height: 38px;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 9px 4px;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
  }
  .banished-row span {
    flex: 1;
  }
  .banished-row small {
    color: var(--muted);
  }
  .banished-row:hover {
    background: var(--surface-hover);
  }
  .dialog-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-top: 18px;
  }
</style>
