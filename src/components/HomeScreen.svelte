<script lang="ts">
  import {
    GripVertical,
    ArrowUp,
    ArrowDown,
    ArrowUpRight,
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
    cardWidth,
    ready,
    error,
    draggable = true,
    onorder,
    onbrowse,
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
    cardWidth: number;
    ready: boolean;
    error?: string;
    draggable?: boolean;
    onorder: (ids: string[]) => void;
    onbrowse: (id: string) => void;
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
  function move(moving: string, target: string, below: boolean) {
    const next = moveHomeRow(
      shelves.map((s) => s.id),
      moving,
      target,
      below,
    );
    onorder(next);
    announcement = `Moved ${shelves.find((s) => s.id === moving)?.title ?? "row"}`;
  }
  function finish() {
    dragging = "";
    over = "";
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
      <p>Drag a row title to arrange your shelves.</p>
    </div>
    <div>
      <button class="text-button" onclick={() => onorder([])}
        >Reset row order</button
      ><button class="secondary-button" onclick={() => onbrowse("activity")}
        ><Library size={15} />Open Library</button
      >
    </div>
  </div>
  <span class="sr-only" role="status">{announcement}</span>
  {#if error}<div class="home-empty">
      <p role="alert">{error}</p>
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
                aria-label={`Move ${shelf.title} row`}
                title="Drag to reorder; use the arrows to move a row with the keyboard"
                ondragstart={(event) => {
                  event.stopPropagation();
                  if (!event.dataTransfer) return;
                  dragging = shelf.id;
                  event.dataTransfer.setData(HOME_ROW_DRAG, shelf.id);
                  event.dataTransfer.effectAllowed = "move";
                }}
                ondragend={finish}
                ><GripVertical size={16} />{shelf.title}</button
              ><span class="shelf-count">{shelf.games.length}</span>
            </h2>
            <p>{shelf.description}</p>
          </div>
          <div class="shelf-controls">
            <button
              class="row-move"
              aria-label={`Move ${shelf.title} up`}
              disabled={index === 0}
              onclick={() => move(shelf.id, shelves[index - 1].id, false)}
              ><ArrowUp size={13} /></button
            ><button
              class="row-move"
              aria-label={`Move ${shelf.title} down`}
              disabled={index === shelves.length - 1}
              onclick={() => move(shelf.id, shelves[index + 1].id, true)}
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
  .home-heading p {
    color: var(--muted);
    font-size: 11px;
    margin-top: 6px;
  }
  .home-heading > div:last-child {
    display: flex;
    align-items: center;
    gap: 18px;
  }
  .home-shelf {
    position: relative;
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
  .shelf-heading h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 15px;
    font-weight: 600;
  }
  .shelf-title {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 2px 0;
    cursor: grab;
  }
  .shelf-title:active {
    cursor: grabbing;
  }
  .shelf-title :global(svg) {
    color: var(--faint);
  }
  .shelf-count {
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
  .row-move:focus-visible {
    opacity: 1;
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
