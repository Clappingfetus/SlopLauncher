<script lang="ts">
  import { Plus, X, ChevronDown } from "@lucide/svelte";
  import { mergeTags, suggestTags, tagKey, type TagOption } from "../lib/tags";
  let {
    tags,
    customTags = [],
    suggestions = [],
    onchange,
    onfilter,
  }: {
    tags: string[];
    customTags?: string[];
    suggestions?: TagOption[];
    onchange: (tags: string[]) => void;
    onfilter: (tag: string) => void;
  } = $props();
  const listId = $props.id();
  let tagInput: HTMLInputElement;
  let draft = $state(""),
    error = $state(""),
    expanded = $state(false);
  let tagKeys = $derived(new Set(tags.map(tagKey)));
  let customKeys = $derived(new Set(customTags.map(tagKey)));
  let availableSuggestions = $derived(
    suggestTags(suggestions, draft, tagKeys),
  );
  let shown = $derived(expanded ? tags : tags.slice(0, 16));
  function add(event: SubmitEvent) {
    event.preventDefault();
    tagInput.focus();
    error = "";
    const label = mergeTags([draft])[0];
    if (!label) {
      error = "Enter a tag of up to 80 characters.";
      return;
    }
    if (customKeys.has(tagKey(label))) {
      error = "You’ve already added this custom tag.";
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
  function remove(tag: string) {
    const key = tagKey(tag);
    onchange(customTags.filter((item) => tagKey(item) !== key));
    if (tagKey(draft) === key) error = "";
    tagInput.focus();
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
              onclick={() => remove(tag)}
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
          : `Show all ${tags.length} tags`}<ChevronDown
          size={12}
          style={expanded ? "transform: rotate(180deg)" : undefined}
        /></button
      >{/if}
  {:else}<p class="tags-empty">
      Tags appear as metadata is found. You can also add your own.
    </p>{/if}
  <form class="add-tag-form" onsubmit={add}>
    <input
      aria-label="Custom tag"
      bind:this={tagInput}
      bind:value={draft}
      oninput={() => (error = "")}
      aria-invalid={error ? true : undefined}
      aria-describedby={error ? `${listId}-error` : undefined}
      maxlength="80"
      placeholder="Add a tag…"
      list={availableSuggestions.length ? listId : undefined}
      autocomplete="off"
    />
    {#if availableSuggestions.length}<datalist id={listId}
        >{#each availableSuggestions as tag (tag.key)}<option value={tag.label}
          ></option>{/each}</datalist
      >{/if}
    <button
      class="secondary-button"
      aria-label="Add tag"
      disabled={!draft.trim()}><Plus size={13} />Add</button
    >
  </form>
  {#if error}<p id={`${listId}-error`} class="form-error" role="alert">{error}</p>{/if}
</section>
