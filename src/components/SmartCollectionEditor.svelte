<script lang="ts">
  import { tick, untrack } from "svelte";
  import {
    Plus,
    Trash2,
    Tags,
    HardDrive,
    CalendarDays,
    Clock3,
    Timer,
    Star,
    FolderHeart,
    Gamepad2,
    ArrowRight,
    Check,
  } from "@lucide/svelte";
  import {
    sourceNames,
    type Game,
    type GameCollection,
    type RuleField,
    type SmartGroup,
  } from "../lib/types";
  import {
    fields,
    operators,
    defaultCondition,
    createSmartGroupPreview,
    validateGroups,
  } from "../lib/grouping";
  import {
    smartCollectionValue,
    legacySmartCollection,
  } from "../lib/smart-collections";
  import { libraryTags, suggestTags, tagKey, tagOptions } from "../lib/tags";
  let {
    initial,
    games,
    now = Date.now(),
    legacy = [],
    onsave,
    oncancel,
    ondelete,
  }: {
    initial?: GameCollection;
    games: Game[];
    now?: number;
    legacy?: SmartGroup[];
    onsave: (value: GameCollection) => void;
    oncancel: () => void;
    ondelete?: () => void;
  } = $props();
  const start = untrack(() =>
    initial ? structuredClone($state.snapshot(initial)) : undefined,
  );
  const originalRule = start?.rule?.groups.find(
    (g) => `smart:${g.id}` === start.rule?.groupId,
  );
  let base = $state<GameCollection | undefined>(start);
  let name = $state(start?.name ?? "");
  let rule = $state<SmartGroup>(
    originalRule ?? {
      id: crypto.randomUUID(),
      name: "",
      match: "all",
      conditions: [],
    },
  );
  let error = $state("");
  let deleting = $state(false);
  let form = $state<HTMLFormElement>();
  const previewLimit = 8;
  const icons = {
    tags: Tags,
    size: HardDrive,
    release: CalendarDays,
    played: Clock3,
    added: CalendarDays,
    hltb: Timer,
    rating: Star,
  };
  const hints: Record<RuleField, string> = {
    tags: "Genres count as tags. Separate multiple tags with commas.",
    size: "Uses the largest installed copy, in GiB. Missing sizes match only ‘Is unknown’.",
    release:
      "Dates with only a year match when the entire year fits your range.",
    played: "Uses the latest known Steam or SlopLauncher activity.",
    added: "The date a game first joined your library.",
    hltb: "HowLongToBeat Main + Extras, in hours. Missing times match only ‘Is unknown’.",
    rating:
      "Consensus score from 1–10, before half-star rounding. Unrated games match only ‘Is unknown’.",
  };
  let previewRules = $derived.by(() => {
    if (!rule.conditions.length)
      return { rules: null, reason: "Add a rule to begin." };
    try {
      const previewGroup = { ...rule, name: "this collection" };
      const rules = base?.rule?.groups.length
        ? base.rule.groups.map((g) =>
            g.id === rule.id ? { ...previewGroup, name: g.name } : g,
          )
        : [previewGroup];
      return { rules: validateGroups(rules), reason: "" };
    } catch (e) {
      return {
        rules: null,
        reason: e instanceof Error ? e.message : "Review the rule values.",
      };
    }
  });
  let buildPreview = $derived(createSmartGroupPreview(games, now));
  let preview = $derived(
    previewRules.rules
      ? buildPreview(previewRules.rules, rule.id, previewLimit)
      : null,
  );
  let tags = $derived(
    tagOptions(libraryTags(games).filter((label) => !label.includes(","))),
  );
  function ruleTagSuggestions(value: string) {
    // Keep the existing comma-separated terms when completing the last tag.
    const prefix = value.slice(0, value.lastIndexOf(",") + 1);
    const tail = value.slice(prefix.length);
    const spacing = tail.match(/^\s*/)?.[0] ?? "";
    const existing = new Set(prefix.split(",").map(tagKey));
    return suggestTags(tags, tail, existing).map(
      ({ label }) => prefix + spacing + label,
    );
  }
  async function add(field: RuleField) {
    const first = !rule.conditions.length;
    const condition = defaultCondition(field);
    rule.conditions = [...rule.conditions, condition];
    error = "";
    await tick();
    const row = form?.querySelector(`[data-condition-id="${condition.id}"]`);
    row?.querySelector<HTMLElement>(first ? "input" : "select")?.focus();
  }
  function reuse(index: number) {
    try {
      const value = legacySmartCollection($state.snapshot(legacy), index);
      base = value;
      name = value.name;
      rule = structuredClone(value.rule!.groups[index]);
      error = "";
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    }
  }
  function save(event: SubmitEvent) {
    event.preventDefault();
    error = "";
    try {
      onsave(
        smartCollectionValue(
          start?.id ?? base?.id ?? crypto.randomUUID(),
          name,
          $state.snapshot(rule),
          base,
        ),
      );
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    }
  }
</script>

<form bind:this={form} class="smart-collection-editor" onsubmit={save}>
  <div class="smart-workspace">
    <div class="smart-builder">
      <label class="collection-name"
        >Collection name<input
          aria-label="Smart collection name"
          bind:value={name}
          placeholder="e.g. Short adventures"
          maxlength="80"
          required
          autocomplete="off"
        /></label
      >
      <div class="rule-heading">
        <div>
          <h3>Rules</h3>
          <p>Membership updates as your library changes.</p>
        </div>
        {#if rule.conditions.length}<label class="match-mode"
            >Match<select
              aria-label="Match collection rules"
              bind:value={rule.match}
              ><option value="all">All rules</option><option value="any"
                >Any rule</option
              ></select
            ></label
          >{/if}
      </div>
      {#if !rule.conditions.length}<div class="rule-empty">
          <span class="empty-rule-icon"><Plus size={22} /></span><strong
            >Start with one rule</strong
          >
          <p>Pick a detail below. You can combine more rules later.</p>
          <div class="rule-choices">
            {#each fields as [field, label]}{@const Icon = icons[field]}<button
                type="button"
                onclick={() => add(field)}
                ><Icon size={16} /><span>{label}</span><Plus
                  size={13}
                /></button
              >{/each}
          </div>
        </div>
      {:else}
        <div class="conditions-list">
          {#each rule.conditions as condition, ruleIndex (condition.id)}
            {@const date =
              condition.field === "release" ||
              ((condition.field === "added" || condition.field === "played") &&
                !["within", "older", "known", "unknown"].includes(
                  condition.op,
                ))}
            <section
              class="condition-card"
              data-condition-id={condition.id}
              aria-label={`Rule ${ruleIndex + 1}`}
            >
              <div class="condition-heading">
                <span class="condition-number">{ruleIndex + 1}</span><strong
                  >{fields.find(([f]) => f === condition.field)?.[1]}</strong
                >
              </div>
              <div
                class="condition-controls"
                role="group"
                aria-label={`Condition ${ruleIndex + 1}`}
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
                  >{#each fields as [value, label]}<option {value}
                      >{label}</option
                    >{/each}</select
                >
                <select aria-label="Rule operator" bind:value={condition.op}
                  >{#each operators(condition.field) as [value, label]}<option
                      {value}>{label}</option
                    >{/each}</select
                >
                {#if !["known", "unknown"].includes(condition.op)}
                  <div
                    class="rule-values"
                    class:range={condition.op === "between"}
                  >
                    <input
                      aria-label="Rule value"
                      list={condition.field === "tags"
                        ? `smart-collection-tags-${condition.id}`
                        : undefined}
                      type={condition.field === "tags"
                        ? "text"
                        : date
                          ? "date"
                          : "number"}
                      min={date
                        ? undefined
                        : condition.field === "rating"
                          ? 1
                          : 0}
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
                      oninput={(event) =>
                        (condition.value = event.currentTarget.value)}
                      required
                    />
                    {#if condition.field === "tags"}
                      <datalist id={`smart-collection-tags-${condition.id}`}>
                        {#each ruleTagSuggestions(condition.value ?? "") as value (value)}
                          <option {value}></option>
                        {/each}
                      </datalist>
                    {/if}
                    {#if condition.op === "between"}<span aria-hidden="true"
                        >to</span
                      ><input
                        aria-label="Rule upper value"
                        max={condition.field === "rating" ? 10 : undefined}
                        type={date ? "date" : "number"}
                        min={date
                          ? undefined
                          : condition.field === "rating"
                            ? 1
                            : 0}
                        step={date ? undefined : "any"}
                        value={condition.upper ?? ""}
                        oninput={(event) =>
                          (condition.upper = event.currentTarget.value)}
                        required
                      />{/if}
                  </div>
                  {#if condition.field !== "tags" && !date}<span
                      class="rule-unit"
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
                  aria-label={`Remove condition ${ruleIndex + 1} from collection`}
                  title="Remove this rule"
                  onclick={() =>
                    (rule.conditions = rule.conditions.filter(
                      (c) => c.id !== condition.id,
                    ))}><Trash2 size={14} /></button
                >
              </div>
              <p class="condition-hint">{hints[condition.field]}</p>
            </section>{/each}
        </div>
        <div class="add-rule-bar">
          <button
            type="button"
            class="text-button"
            disabled={rule.conditions.length >= 12}
            onclick={() => add("tags")}
            ><Plus size={14} />Add another rule</button
          ><span>{rule.conditions.length}/12 rules</span>
        </div>
      {/if}
      {#if legacy.length && !start}<details class="legacy-rules">
          <summary>Reuse a previously saved rule</summary>
          <div>
            {#each legacy as group, index}<button
                type="button"
                onclick={() => reuse(index)}
                >{group.name}<ArrowRight size={13} /></button
              >{/each}
          </div>
        </details>{/if}
      {#if error}<p class="form-error" role="alert">{error}</p>{/if}
    </div>
    <aside class="smart-preview" aria-label="Collection preview">
      <div class="preview-heading">
        <span><FolderHeart size={17} />Live preview</span>
      </div>
      {#if !preview}<div class="preview-empty">
          <Gamepad2 size={30} strokeWidth={1} /><strong
            >{rule.conditions.length
              ? "Check the rules"
              : "No rules yet"}</strong
          >
          <p>{previewRules.reason}</p>
        </div>
      {:else if !preview.count}<div class="preview-empty">
          <Check size={28} strokeWidth={1} /><strong>No matches yet</strong>
          <p>
            You can still save this collection. Future matches will join
            automatically.
          </p>
        </div>
      {:else}
        <div class="preview-games" role="region" aria-label="Sample matches">
          {#each preview.sample as game (game.id)}<div class="preview-game">
              <span class="preview-game-icon"><Gamepad2 size={15} /></span><span
                ><strong>{game.title}</strong><small
                  >{[
                    ...new Set(
                      game.copies.map((c) => sourceNames[c.source] ?? c.source),
                    ),
                  ].join(" · ")}</small
                ></span
              >
            </div>{/each}
        </div>
        {#if preview.count > preview.sample.length}<p class="preview-more">
            +{(preview.count - preview.sample.length).toLocaleString()} more
          </p>{/if}{/if}
      <p class="preview-note">
        Matches your whole library. Search and page filters do not limit this
        collection.
      </p>
    </aside>
  </div>
  <footer class="smart-footer">
    <span class="match-count" role="status" aria-atomic="true">
      {#if preview}
        {preview.count.toLocaleString()} matching {preview.count === 1
          ? "game"
          : "games"}
      {/if}
    </span>
    {#if start && ondelete}<div>
        {#if deleting}<button
            type="button"
            class="danger-button"
            onclick={ondelete}>Confirm delete</button
          ><button
            type="button"
            class="text-button"
            onclick={() => (deleting = false)}>Keep collection</button
          >{:else}<button
            type="button"
            class="text-button"
            onclick={() => (deleting = true)}
            ><Trash2 size={14} />Delete collection</button
          >{/if}
      </div>{/if}
    <div>
      <button type="button" class="secondary-button" onclick={oncancel}
        >Cancel</button
      ><button
        type="submit"
        class="primary-button"
        disabled={!name.trim() || !preview}
        >{start ? "Save collection" : "Create collection"}<ArrowRight
          size={14}
        /></button
      >
    </div>
  </footer>
</form>

<style>
  .smart-workspace {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 280px;
    gap: 26px;
    align-items: start;
  }
  .smart-builder {
    min-width: 0;
  }
  .collection-name {
    display: flex;
    flex-direction: column;
    gap: 9px;
    font-size: 12px;
    color: var(--muted);
  }
  .collection-name input {
    width: 100%;
    font-size: 16px;
    padding: 13px 14px;
    background: var(--bg);
    border-radius: 9px;
    border: 1px solid var(--line);
    color: var(--text);
  }
  .rule-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 26px 0 16px;
  }
  .rule-heading h3 {
    font-size: 13px;
    margin: 0;
  }
  .rule-heading p {
    font-size: 11px;
    color: var(--muted);
    margin: 5px 0 0;
  }
  .match-mode {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 11px;
    color: var(--muted);
  }
  .match-mode select {
    font-size: 11px;
    padding: 7px;
  }
  .rule-empty {
    border: 1px dashed var(--line);
    border-radius: 12px;
    padding: 22px 18px;
    text-align: center;
    background: var(--bg);
  }
  .empty-rule-icon {
    display: grid;
    place-items: center;
    margin: 0 auto 12px;
    width: 36px;
    height: 36px;
    background: var(--surface);
    color: var(--accent);
    border-radius: 50%;
  }
  .rule-empty > strong {
    font-size: 13px;
  }
  .rule-empty > p {
    font-size: 11px;
    color: var(--muted);
    line-height: 1.6;
    margin: 6px 0 18px;
  }
  .rule-choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
  }
  .rule-choices button {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 11px 10px;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: 7px;
    background: var(--surface);
    color: var(--muted);
    font-size: 11px;
    cursor: pointer;
    transition:
      border-color 0.12s,
      color 0.12s;
  }
  .rule-choices button:hover {
    border-color: var(--accent);
    color: var(--text);
  }
  .rule-choices button span {
    flex: 1;
  }
  .conditions-list {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .condition-card {
    padding: 15px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg);
  }
  .condition-heading {
    display: flex;
    gap: 8px;
    align-items: center;
    margin-bottom: 12px;
    font-size: 11px;
  }
  .condition-number {
    width: 20px;
    height: 20px;
    border: 1px solid var(--line);
    display: grid;
    place-items: center;
    border-radius: 6px;
    color: var(--accent);
    font-size: 10px;
  }
  .condition-controls {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) 28px;
    gap: 9px;
    align-items: center;
  }
  .condition-controls input {
    width: 100%;
    min-width: 0;
    font-size: 12px;
    padding: 9px;
    border-radius: 6px;
  }
  .condition-controls select,
  .match-mode select {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 6px;
    min-height: 35px;
  }
  .rule-values {
    grid-column: 1 / -1;
  }
  .rule-values.range {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr);
    gap: 9px;
    align-items: center;
    margin-top: 8px;
  }
  .rule-values.range input {
    margin-top: 0;
  }
  .rule-values > span {
    color: var(--muted);
    font-size: 11px;
  }
  .condition-controls > button {
    grid-row: 1;
    grid-column: 3;
    padding: 5px;
  }
  .condition-controls .rule-unit {
    font-size: 10px;
    color: var(--faint);
    grid-column: 2/-1;
    justify-self: end;
  }
  .condition-hint {
    font-size: 10px;
    line-height: 1.6;
    color: var(--faint);
    margin: 9px 0 0;
  }
  .add-rule-bar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin: 14px 0;
    font-size: 10px;
    color: var(--faint);
  }
  .add-rule-bar button {
    font-size: 11px;
  }
  .smart-preview {
    position: sticky;
    top: 56px;
    border: 1px solid var(--line);
    border-radius: 12px;
    background: var(--bg);
    overflow: hidden;
  }
  .preview-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 17px 16px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
  }
  .preview-heading > span:first-child {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .preview-heading :global(svg) {
    color: var(--accent);
  }
  .preview-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    min-height: 225px;
    padding: 25px 22px;
    text-align: center;
    color: var(--faint);
    gap: 12px;
  }
  .preview-empty strong {
    font-size: 12px;
    font-weight: 500;
    color: var(--muted);
  }
  .preview-empty p {
    font-size: 11px;
    line-height: 1.7;
    margin: 0;
  }
  .preview-games {
    padding: 0 16px;
    max-height: clamp(153px, calc(100vh - 385px), 408px);
    overflow-y: auto;
  }
  .preview-games:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  .preview-game {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 51px;
    border-bottom: 1px solid var(--line);
  }
  .preview-game:last-child {
    border-bottom: 0;
  }
  .preview-game-icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 34px;
    background: var(--surface);
    border-radius: 6px;
    color: var(--faint);
    flex-shrink: 0;
  }
  .preview-game > span:last-child {
    min-width: 0;
  }
  .preview-game strong {
    display: block;
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .preview-game small {
    display: block;
    color: var(--faint);
    font-size: 9px;
    margin-top: 4px;
  }
  .preview-more {
    font-size: 10px;
    color: var(--muted);
    margin: 10px 16px 16px;
  }
  .preview-note {
    font-size: 10px;
    line-height: 1.6;
    padding: 13px 16px;
    margin: 0;
    border-top: 1px solid var(--line);
    color: var(--faint);
  }
  .legacy-rules {
    border-top: 1px solid var(--line);
    margin-top: 20px;
    padding-top: 15px;
    font-size: 11px;
    color: var(--muted);
  }
  .legacy-rules summary {
    cursor: pointer;
  }
  .legacy-rules > div {
    display: grid;
    gap: 5px;
    margin-top: 12px;
  }
  .legacy-rules button {
    display: flex;
    justify-content: space-between;
    align-items: center;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 9px 11px;
    color: var(--text);
    cursor: pointer;
    font-size: 11px;
  }
  .smart-footer {
    position: sticky;
    bottom: -25px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    margin: 24px -27px -25px;
    padding: 18px 28px;
    background: var(--surface);
    border-top: 1px solid var(--line);
    z-index: 1;
  }
  .match-count {
    margin-inline-end: auto;
    color: var(--text);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .smart-footer > div {
    display: flex;
    gap: 8px;
  }
  .smart-footer button {
    font-size: 12px;
  }
  @media (max-width: 760px) {
    .smart-workspace {
      grid-template-columns: minmax(0, 1fr);
    }
    .smart-preview {
      position: static;
    }
    .preview-empty {
      min-height: 130px;
    }
    .smart-footer {
      flex-wrap: wrap;
    }
    .smart-footer > div {
      flex-wrap: wrap;
    }
    .rule-choices {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .rule-choices button {
      transition: none;
    }
  }
</style>
