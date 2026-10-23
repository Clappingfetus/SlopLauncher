<script lang="ts">
  import { untrack } from "svelte";
  import { Plus, Trash2, ArrowUp, ArrowDown, FolderPlus } from "@lucide/svelte";
  import type { Game, RuleField, SmartGroup } from "../lib/types";
  import {
    fields,
    operators,
    defaultCondition,
    groupGames,
    validateGroups,
  } from "../lib/grouping";
  let {
    initial,
    games,
    onsave,
    oncancel,
    oncollection,
  }: {
    initial: SmartGroup[];
    games: Game[];
    onsave: (rules: SmartGroup[]) => void;
    oncancel: () => void;
    oncollection?: (
      groups: SmartGroup[],
      id: string,
      draft: SmartGroup[],
    ) => void;
  } = $props();
  let draft = $state<SmartGroup[]>(
    untrack(() => structuredClone($state.snapshot(initial))),
  );
  let error = $state("");
  let preview = $derived(groupGames(games, "smart", draft));
  function addGroup() {
    draft = [
      ...draft,
      {
        id: crypto.randomUUID(),
        name: `Group ${draft.length + 1}`,
        match: "all",
        conditions: [defaultCondition()],
      },
    ];
  }
  function move(index: number, delta: number) {
    const next = [...draft];
    [next[index], next[index + delta]] = [next[index + delta], next[index]];
    draft = next;
  }
  function saveAsCollection(index: number) {
    try {
      const rules = validateGroups($state.snapshot(draft.slice(0, index + 1)));
      oncollection?.(
        rules,
        `smart:${rules[index].id}`,
        structuredClone($state.snapshot(draft)),
      );
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    }
  }
  function save(event: SubmitEvent) {
    event.preventDefault();
    try {
      const rules = validateGroups($state.snapshot(draft));
      onsave(rules);
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    }
  }
</script>

<form class="smart-group-editor" onsubmit={save}>
  <p class="muted">
    Groups run from top to bottom. The first matching group wins; unmatched
    games stay in Other. New games update automatically.
  </p>
  {#each draft as group, index (group.id)}
    <section class="smart-group-rule" aria-label={`Smart group ${index + 1}`}>
      <div class="smart-group-title">
        <label
          >Group name<input
            aria-label={`Group name ${index + 1}`}
            bind:value={group.name}
            maxlength="80"
            required
          /></label
        >
        <span class="smart-count"
          >{preview.find((g) => g.id === `smart:${group.id}`)?.games.length ??
            0} games</span
        >
        <button
          type="button"
          class="icon-button"
          aria-label={`Move group ${index + 1} up`}
          disabled={index === 0}
          onclick={() => move(index, -1)}><ArrowUp size={15} /></button
        >
        <button
          type="button"
          class="icon-button"
          aria-label={`Move group ${index + 1} down`}
          disabled={index === draft.length - 1}
          onclick={() => move(index, 1)}><ArrowDown size={15} /></button
        >
        <button
          type="button"
          class="icon-button"
          aria-label={`Delete group ${index + 1}`}
          onclick={() => (draft = draft.filter((g) => g.id !== group.id))}
          ><Trash2 size={15} /></button
        >
      </div>
      {#if oncollection}<button
          type="button"
          class="text-button"
          onclick={() => saveAsCollection(index)}
          aria-label={`Save group ${index + 1} as Collection`}
          ><FolderPlus size={14} />Save as Collection</button
        >{/if}
      <label class="rule-match"
        >Match<select
          aria-label={`Match conditions for group ${index + 1}`}
          bind:value={group.match}
          ><option value="all">All conditions</option><option value="any"
            >Any condition</option
          ></select
        ></label
      >
      {#each group.conditions as condition, ruleIndex (condition.id)}
        {@const date =
          condition.field === "release" ||
          ((condition.field === "added" || condition.field === "played") &&
            !["within", "older", "known", "unknown"].includes(condition.op))}
        <div
          class="smart-condition"
          role="group"
          aria-label={`Condition ${ruleIndex + 1} in group ${index + 1}`}
        >
          <select
            aria-label="Rule field"
            value={condition.field}
            onchange={(event) => {
              const next = defaultCondition(
                event.currentTarget.value as RuleField,
              );
              Object.assign(condition, { ...next, id: condition.id });
            }}
            >{#each fields as [value, label]}<option {value}>{label}</option
              >{/each}</select
          >
          <select aria-label="Rule operator" bind:value={condition.op}
            >{#each operators(condition.field) as [value, label]}<option {value}
                >{label}</option
              >{/each}</select
          >
          {#if !["known", "unknown"].includes(condition.op)}
            <input
              aria-label="Rule value"
              type={condition.field === "tags"
                ? "text"
                : date
                  ? "date"
                  : "number"}
              min={date ? undefined : condition.field === "rating" ? 1 : 0}
              max={date
                ? undefined
                : condition.field === "rating"
                  ? 10
                  : 1000000}
              step={date ? undefined : "any"}
              placeholder={condition.field === "tags"
                ? "Tags, separated by commas"
                : condition.field === "size"
                  ? "GiB"
                  : condition.field === "rating"
                    ? "Score / 10"
                    : condition.field === "hltb"
                      ? "Hours"
                      : "Days"}
              value={condition.value ?? ""}
              oninput={(event) => (condition.value = event.currentTarget.value)}
              required
            />
            {#if condition.op === "between"}<span class="muted">to</span><input
                aria-label="Rule upper value"
                max={condition.field === "rating" ? 10 : undefined}
                type={date ? "date" : "number"}
                min={date ? undefined : condition.field === "rating" ? 1 : 0}
                step={date ? undefined : "any"}
                value={condition.upper ?? ""}
                oninput={(event) =>
                  (condition.upper = event.currentTarget.value)}
                required
              />{/if}
            {#if condition.field !== "tags" && !date}<span class="rule-unit"
                >{condition.field === "size"
                  ? "GiB"
                  : condition.field === "rating"
                    ? "Score / 10"
                    : condition.field === "hltb"
                      ? "hours"
                      : "days"}</span
              >{/if}
          {/if}
          <button
            type="button"
            class="icon-button"
            aria-label={`Remove condition ${ruleIndex + 1} from group ${index + 1}`}
            disabled={group.conditions.length === 1}
            onclick={() =>
              (group.conditions = group.conditions.filter(
                (c) => c.id !== condition.id,
              ))}><Trash2 size={14} /></button
          >
        </div>
      {/each}
      <button
        type="button"
        class="text-button"
        disabled={group.conditions.length >= 12}
        onclick={() =>
          (group.conditions = [...group.conditions, defaultCondition()])}
        ><Plus size={13} />Add condition</button
      >
    </section>
  {/each}
  <button
    type="button"
    class="secondary-button"
    disabled={draft.length >= 32}
    onclick={addGroup}><Plus size={14} />Add group</button
  >
  <p class="field-hint">
    Consensus ratings use the precise 60% users / 40% critics score (1–10),
    before half-star rounding. Decimal thresholds such as 8.5 are supported;
    unrated games match only “Is unknown”. Sizes use the largest reported
    installed copy, in GiB. Missing sizes or HLTB times match only “Is unknown”.
    Year-only releases match dates only when the entire year fits the range.
    Times use Main + Extras; tags include genres.
  </p>
  <p class="muted">
    Other: {preview.find((g) => g.id === "other")?.games.length ?? 0} games
  </p>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
  <div class="dialog-actions">
    <button type="button" class="secondary-button" onclick={oncancel}
      >Cancel</button
    ><button type="submit" class="primary-button">Save smart groups</button>
  </div>
</form>
