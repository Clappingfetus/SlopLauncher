<script lang="ts">
  import { tick } from "svelte";
  import {
    GripVertical,
    ArrowUp,
    ArrowDown,
    ArrowUpRight,
    ChevronDown,
    Library,
  } from "@lucide/svelte";
  import HomeShelf from "./HomeShelf.svelte";
  import {
    HOME_ROW_DRAG,
    moveHomeRow,
    type HomeShelf as Shelf,
  } from "../lib/home";
  import type { Game } from "../lib/types";
  let {
    shelves,
    orderChanged,
    cardWidth,
    ready,
    error,
    draggable = true,
    onorder,
    onbrowse,
    onlibrary,
    onretry,
    onopen,
    onfavorite,
    onhide,
    onlaunch,
    oncontext,
    versionCount,
    onsources,
    initialTop = 0,
    scrolls = new Map<string, number>(),
    onscroll,
  }: {
    shelves: Shelf[];
    orderChanged: boolean;
    cardWidth: number;
    ready: boolean;
    error?: string;
    draggable?: boolean;
    onorder: (ids: string[]) => void;
    onbrowse: (id: string) => void;
    onlibrary: () => void;
    onretry: () => void;
    onopen: (g: Game) => void;
    onfavorite: (g: Game) => void;
    onhide: (g: Game) => void;
    onlaunch: (g: Game) => void;
    oncontext: (event: MouseEvent, g: Game) => void;
    versionCount: (g: Game) => number;
    onsources: () => void;
    initialTop?: number;
    scrolls?: Map<string, number>;
    onscroll?: (top: number) => void;
  } = $props();
  let dragging = $state(""),
    over = $state(""),
    after = $state(false),
    page = $state<HTMLElement>();
  let count = $derived(
    shelves.find((s) => s.id === "added")?.games.length ?? 0,
  );
  let announcement = $state("");
  function move(moving: string, target: string, below: boolean, control?: HTMLButtonElement) {
    if (moving === target) return;
    const next = moveHomeRow(
      shelves.map((s) => s.id),
      moving,
      target,
      below,
    );
    onorder(next);
    const index = next.indexOf(moving);
    const title = shelves.find((s) => s.id === moving)?.title ?? "Shelf";
    announcement = `${title} moved to row ${index + 1} of ${next.length}.`;
    if (control) void tick().then(() => {
      const focusTarget = control.disabled
        ? control.closest("section")?.querySelector<HTMLButtonElement>(".shelf-title")
        : control;
      focusTarget?.focus();
    });
  }
  function finish() {
    dragging = "";
    over = "";
  }
  function jumpToShelf(select: HTMLSelectElement) {
    const section = page?.querySelector<HTMLElement>(
      `[data-home-shelf="${CSS.escape(select.value)}"]`,
    );
    select.value = "";
    section?.scrollIntoView({ block: "start" });
    section?.querySelector<HTMLButtonElement>(".shelf-title")?.focus({
      preventScroll: true,
    });
  }
  $effect(() => {
    if (page) page.scrollTop = initialTop;
  });
</script>

<main
  class="home-page"
  aria-label="Home"
  bind:this={page}
  onscroll={(event) => onscroll?.(event.currentTarget.scrollTop)}
>
  <div class="home-heading">
    <div>
      <h1>Home <span>{count}</span></h1>
    </div>
    <div>
      {#if ready && count && !error}
        <label class="shelf-jump sort-control">
          <select
            aria-label="Jump to shelf"
            onchange={(event) => jumpToShelf(event.currentTarget)}
          >
            <option value="" selected disabled>Jump to shelf</option>
            {#each shelves as shelf (shelf.id)}
              <option value={shelf.id}>{shelf.title} ({shelf.games.length})</option>
            {/each}
          </select><ChevronDown size={12} />
        </label>
      {/if}
      <button
        class="text-button"
        aria-disabled={!ready || !count || !orderChanged}
        onclick={() => {
          if (!ready || !count || !orderChanged) return;
          onorder([]);
          announcement = "Default Home row order restored.";
        }}
        >Reset row order</button
      ><button class="secondary-button" onclick={onlibrary}
        ><Library size={15} />Open Library</button
      >
    </div>
  </div>
  <span class="sr-only" role="status">{announcement}</span>
  {#if error}<div class="home-empty">
      <p role="alert">{error}</p>
      <button class="primary-button" onclick={onretry}>Retry opening library</button>
      <button class="secondary-button" onclick={onsources}>Open Sources</button>
    </div>
  {:else if !ready}<div class="home-empty">Opening your library…</div>
  {:else if !count}<div class="home-empty">
      <h2>Your library is ready</h2>
      <p>Connect a source or add a game to fill your shelves.</p>
      <button class="primary-button" onclick={onsources}>Connect sources</button
      >
    </div>
  {:else}
    {#each shelves as shelf, index (shelf.id)}
      <section
        class="home-shelf"
        class:row-dragging={dragging === shelf.id}
        class:drop-before={over === shelf.id && !after && dragging !== shelf.id}
        class:drop-after={over === shelf.id && after && dragging !== shelf.id}
        data-home-shelf={shelf.id}
        aria-label={shelf.title}
        ondragover={(event) => {
          if (!dragging || !event.dataTransfer?.types.includes(HOME_ROW_DRAG))
            return;
          event.preventDefault();
          event.dataTransfer.dropEffect = "move";
          over = shelf.id;
          const r = event.currentTarget.getBoundingClientRect();
          after = event.clientY > r.top + r.height / 2;
        }}
        ondragleave={(event) => {
          if (
            !(event.relatedTarget instanceof Node) ||
            !event.currentTarget.contains(event.relatedTarget)
          )
            over = "";
        }}
        ondrop={(event) => {
          if (
            !dragging ||
            event.dataTransfer?.getData(HOME_ROW_DRAG) !== dragging
          )
            return;
          event.preventDefault();
          event.stopPropagation();
          move(dragging, shelf.id, after);
          finish();
        }}
      >
        <div class="shelf-heading">
          <div>
            <h2>
              <button
                class="shelf-title"
                draggable="true"
                aria-label={`Browse ${shelf.title}`}
                title={`${shelf.title} · Click to browse, drag or use the up and down arrow keys to reorder`}
                onclick={() => onbrowse(shelf.id)}
                onkeydown={(event) => {
                  if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return;
                  event.preventDefault();
                  const below = event.key === "ArrowDown";
                  const target = shelves[index + (below ? 1 : -1)];
                  if (target) move(shelf.id, target.id, below, event.currentTarget);
                }}
                ondragstart={(event) => {
                  event.stopPropagation();
                  if (!event.dataTransfer) return;
                  dragging = shelf.id;
                  event.dataTransfer.setData(HOME_ROW_DRAG, shelf.id);
                  event.dataTransfer.effectAllowed = "move";
                }}
                ondragend={finish}
                ><GripVertical size={16} /><span>{shelf.title}</span></button
              ><span class="shelf-count">{shelf.games.length}</span>
            </h2>
            <p>{shelf.description}</p>
          </div>
          <div class="shelf-controls">
            <button
              class="row-move"
              aria-label={`Move ${shelf.title} up`}
              disabled={index === 0}
              onclick={(event) => move(shelf.id, shelves[index - 1].id, false, event.currentTarget)}
              ><ArrowUp size={13} /></button
            ><button
              class="row-move"
              aria-label={`Move ${shelf.title} down`}
              disabled={index === shelves.length - 1}
              onclick={(event) => move(shelf.id, shelves[index + 1].id, true, event.currentTarget)}
              ><ArrowDown size={13} /></button
            ><button class="text-button" onclick={() => onbrowse(shelf.id)}
              >View all<ArrowUpRight size={12} /></button
            >
          </div>
        </div>
        {#if shelf.games.length}<HomeShelf
            games={shelf.games}
            {cardWidth}
            active={index < 2}
            {draggable}
            {onopen}
            {onfavorite}
            {onhide}
            {onlaunch}
            {oncontext}
            {versionCount}
            initialScroll={scrolls.get(shelf.id) ?? 0}
            onscroll={(value) => scrolls.set(shelf.id, value)}
          />
        {:else}<div class="shelf-empty">
            {shelf.id === "played"
              ? "Play a game to start this shelf."
              : shelf.id === "short" || shelf.id === "long"
                ? "Games appear here when their Main + Extras time is available."
                : "No games on this shelf yet."}
          </div>{/if}
      </section>
    {/each}
  {/if}
</main>

<style>
  .home-page {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 30px 28px 36px;
    scrollbar-width: thin;
    scrollbar-color: var(--line) transparent;
    scrollbar-gutter: stable;
    overscroll-behavior: contain;
  }
  .home-heading {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-between;
    align-items: center;
    gap: 20px;
    margin-bottom: 28px;
  }
  .home-heading h1 {
    font-size: 28px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .home-heading h1 span {
    font-size: 11px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 2px 6px;
  }

  .home-heading > div:last-child {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 18px;
  }
  .shelf-jump {
    position: relative;
  }
  .shelf-jump select {
    width: 116px;
    padding: 9px 24px 9px 10px;
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .shelf-jump :global(svg) {
    position: absolute;
    right: 9px;
    pointer-events: none;
  }
  .home-heading .text-button[aria-disabled="true"] {
    color: var(--faint);
    cursor: default;
  }
  .home-shelf {
    position: relative;
    scroll-margin-top: 12px;
    margin-bottom: 26px;
    border-radius: 8px;
  }
  .row-dragging {
    opacity: 0.38;
  }
  .drop-before::before,
  .drop-after::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    height: 3px;
    background: var(--accent);
    border-radius: 4px;
    box-shadow: 0 0 0 3px var(--bg);
    z-index: 8;
  }
  .drop-before::before {
    top: -13px;
  }
  .drop-after::after {
    bottom: -13px;
  }
  .shelf-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    gap: 16px;
  }
  .shelf-heading > div:first-child {
    min-width: 0;
    flex: 1;
  }
  .shelf-heading h2 {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    font-weight: 600;
  }
  .shelf-title {
    min-width: 0;
    max-width: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 2px 0;
    cursor: grab;
  }
  .shelf-title span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .shelf-title:active {
    cursor: grabbing;
  }
  .shelf-title :global(svg) {
    flex-shrink: 0;
    color: var(--faint);
  }
  .shelf-count {
    flex-shrink: 0;
    font-size: 10px;
    color: var(--faint);
    font-weight: 400;
  }
  .shelf-heading p {
    font-size: 10px;
    color: var(--muted);
    margin: 4px 0 0 23px;
  }
  .shelf-controls {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-shrink: 0;
  }
  .row-move {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    opacity: 0;
  }
  .home-shelf:hover .row-move,
  .shelf-heading:focus-within .row-move {
    opacity: 1;
  }
  .shelf-title:hover span,
  .shelf-title:focus-visible span {
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  @media (hover: none) {
    .row-move { opacity: 1; }
  }
  .shelf-controls .text-button {
    font-size: 11px;
    margin-left: 7px;
  }
  .shelf-empty {
    padding: 23px;
    border: 1px dashed var(--line);
    border-radius: 8px;
    font-size: 12px;
    color: var(--muted);
  }
  .home-empty {
    padding: 60px 20px;
    text-align: center;
    display: grid;
    justify-items: center;
    gap: 18px;
  }
  .home-empty p {
    color: var(--muted);
    font-size: 13px;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
  }
</style>
