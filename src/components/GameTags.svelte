<script lang="ts">
  import { Plus, X, ChevronDown } from "@lucide/svelte";
  import { mergeTags, tagKey } from "../lib/tags";
  let {
    gameId,
    tags,
    customTags = [],
    onchange,
    onfilter,
  }: {
    gameId: string;
    tags: string[];
    customTags?: string[];
    onchange: (tags: string[]) => void;
    onfilter: (tag: string) => void;
  } = $props();
  let draft = $state(""),
    error = $state(""),
    expanded = $state(false);
  let customKeys = $derived(new Set(customTags.map(tagKey)));
  let shown = $derived(expanded ? tags : tags.slice(0, 16));
  $effect(() => {
    void gameId;
    draft = "";
    error = "";
    expanded = false;
  });
  function add(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    const label = mergeTags([draft])[0];
    if (!label) {
      error = "Enter a tag of up to 80 characters.";
      return;
    }
    const next = mergeTags(customTags, [label]);
    if (next.length > 64) {
      error = "You can save up to 64 custom tags per game.";
      return;
    }
    onchange(next);
    draft = "";
  }
</script>

<section class="game-tags-section" aria-label="Game tags">
  <h2>
    Tags{#if tags.length}<span>{tags.length}</span>{/if}
  </h2>
  {#if tags.length}<div class="game-tag-list">
      {#each shown as tag (tagKey(tag))}
        <span class="game-tag" class:custom-tag={customKeys.has(tagKey(tag))}>
          <button
            onclick={() => onfilter(tag)}
            aria-label={`Filter by tag ${tag}`}
            title={`Browse games tagged ${tag}`}>{tag}</button
          >
          {#if customKeys.has(tagKey(tag))}<button
              class="remove-tag"
              title="Remove your custom tag"
              aria-label={`Remove custom tag ${tag}`}
              onclick={() =>
                onchange(customTags.filter((t) => tagKey(t) !== tagKey(tag)))}
              ><X size={11} /></button
            >{/if}
        </span>
      {/each}
    </div>
    {#if tags.length > 16}<button
        class="text-button more-tags"
        aria-expanded={expanded}
        onclick={() => (expanded = !expanded)}
      >
        {expanded
          ? "Show fewer tags"
          : `Show all ${tags.length} tags`}<ChevronDown size={12} /></button
      >{/if}
  {:else}<p class="tags-empty">
      Tags appear as metadata is found. You can also add your own.
    </p>{/if}
  <form class="add-tag-form" onsubmit={add}>
    <input
      aria-label="Custom tag"
      bind:value={draft}
      maxlength="80"
      placeholder="Add a tag…"
      autocomplete="off"
    />
    <button
      class="secondary-button"
      aria-label="Add tag"
      disabled={!draft.trim()}><Plus size={13} />Add</button
    >
  </form>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
</section>
