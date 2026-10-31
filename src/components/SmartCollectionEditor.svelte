<script lang="ts">
  import { untrack } from "svelte";
  import {
    Plus,
    Trash2,
    Sparkles,
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
    groupGames,
    validateGroups,
  } from "../lib/grouping";
  import {
    smartCollectionValue,
    legacySmartCollection,
  } from "../lib/smart-collections";
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
  let checked = $derived.by(() => {
    if (!rule.conditions.length)
      return {
        valid: false,
        games: [] as Game[],
        reason: "Add a rule to begin.",
      };
    try {
      const previewGroup = { ...rule, name: name.trim() || "New collection" };
      const rules = base?.rule?.groups.length
        ? base.rule.groups.map((g) =>
            g.id === rule.id ? { ...previewGroup, name: g.name } : g,
          )
        : [previewGroup];
      validateGroups(rules);
      return {
        valid: true,
        games:
          groupGames(games, "smart", rules, now).find(
            (g) => g.id === `smart:${rule.id}`,
          )?.games ?? [],
        reason: "",
      };
    } catch {
      return {
        valid: false,
        games: [] as Game[],
        reason: "Finish the rule values to see matching games.",
      };
    }
  });
  let tags = $derived(
    [...new Set(games.flatMap((g) => [...g.genres, ...(g.tags ?? [])]))]
      .sort((a, b) => a.localeCompare(b))
      .slice(0, 200),
  );
  function add(field: RuleField) {
    rule.conditions = [...rule.conditions, defaultCondition(field)];
    error = "";
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

<form class="smart-collection-editor" onsubmit={save}>
  <div class="smart-intro">
    <span class="smart-emblem"><Sparkles size={22} /></span>
    <div>
      <strong>Choose rules for your collection.</strong>
      <p>
        Choose what belongs. Matching games join automatically as your library
        changes.
      </p>
    </div>
    <span class="live-badge"><span></span>Live collection</span>
  </div>
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
          <p>Combine the things you care about.</p>
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
                  <input
                    aria-label="Rule value"
                    list={condition.field === "tags"
                      ? "smart-collection-tags"
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
                  {#if condition.op === "between"}<span class="muted">to</span
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
        <span><FolderHeart size={17} />Live preview</span><span
          class="preview-count"
          aria-live="polite">{checked.valid ? checked.games.length : "—"}</span
        >
      </div>
      {#if !checked.valid}<div class="preview-empty">
          <Gamepad2 size={30} strokeWidth={1} /><strong
            >Your collection starts here</strong
          >
          <p>{checked.reason}</p>
        </div>
      {:else if !checked.games.length}<div class="preview-empty">
          <Check size={28} strokeWidth={1} /><strong>No matches yet</strong>
          <p>
            You can still save this collection. Future matches will join
            automatically.
          </p>
        </div>
      {:else}<p class="preview-caption">
          {checked.games.length} matching {checked.games.length === 1
            ? "game"
            : "games"} in your library
        </p>
        <div class="preview-games">
          {#each checked.games.slice(0, 8) as game (game.id)}<div
              class="preview-game"
            >
              <span class="preview-game-icon"><Gamepad2 size={15} /></span><span
                ><strong>{game.title}</strong><small
                  >{[
                    ...new Set(game.copies.map((c) => sourceNames[c.source])),
                  ].join(" · ")}</small
                ></span
              >
            </div>{/each}
        </div>
        {#if checked.games.length > 8}<p class="preview-more">
            +{checked.games.length - 8} more games
          </p>{/if}{/if}
      <p class="preview-note">
        Matches your whole library. Search and page filters do not limit this
        collection.
      </p>
    </aside>
  </div>
  <datalist id="smart-collection-tags"
    >{#each tags as tag}<option value={tag}></option>{/each}</datalist
  >
  <footer class="smart-footer">
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
      </div>{:else}<span><Sparkles size={13} />Updates automatically</span>{/if}
    <div>
      <button type="button" class="secondary-button" onclick={oncancel}
        >Cancel</button
      ><button
        type="submit"
        class="primary-button"
        disabled={!name.trim() || !checked.valid}
        >{start ? "Save collection" : "Create collection"}<ArrowRight
          size={14}
        /></button
      >
    </div>
  </footer>
</form>

<style>
  .smart-intro {
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 6px 0 26px;
  }
  .smart-emblem {
    display: grid;
    place-items: center;
    width: 46px;
    height: 46px;
    border: 1px solid var(--line);
    border-radius: 14px;
    color: var(--accent);
    background: var(--surface);
    flex-shrink: 0;
  }
  .smart-intro strong {
    font-size: 14px;
    font-weight: 600;
  }
  .smart-intro p {
    font-size: 12px;
    line-height: 1.6;
    color: var(--muted);
    margin: 5px 0 0;
  }
  .live-badge {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    white-space: nowrap;
    color: var(--muted);
    font-size: 10px;
    border: 1px solid var(--line);
    border-radius: 20px;
    padding: 6px 9px;
  }
  .live-badge > span {
    width: 5px;
    height: 5px;
    background: var(--accent);
    border-radius: 50%;
  }
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
  .condition-controls select,
  .condition-controls select,
  .match-mode select {
    background: var(--surface);
    color: var(--text);
    border: 1px solid var(--line);
    border-radius: 6px;
    min-height: 35px;
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
  .condition-controls input {
    grid-column: 1/-1;
  }
  .condition-controls > button {
    grid-row: 1;
    grid-column: 3;
    padding: 5px;
  }
  .condition-controls > .muted {
    font-size: 10px;
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
    top: 0;
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
  .preview-count {
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    background: var(--surface);
    border-radius: 6px;
    min-width: 29px;
    padding: 4px 6px;
    text-align: center;
    font-size: 12px;
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
  .preview-caption {
    font-size: 10px;
    color: var(--muted);
    margin: 14px 16px 8px;
  }
  .preview-games {
    padding: 0 16px;
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
  .smart-footer > span {
    display: flex;
    align-items: center;
    gap: 7px;
    color: var(--faint);
    font-size: 10px;
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
    .live-badge {
      display: none;
    }
    .smart-footer > span {
      display: none;
    }
    .smart-footer {
      justify-content: flex-end;
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
