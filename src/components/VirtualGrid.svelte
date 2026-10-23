<script lang="ts">
  import { FolderPlus } from "@lucide/svelte";
  import { tick } from "svelte";
  import type { Game, LibraryGroup } from "../lib/types";
  import GameCard from "./GameCard.svelte";
  import { gridLayout, gridRowAt } from "../lib/grid-layout";
  import { setArtworkWindow, clearArtworkWindow } from "../lib/backend";
  import { artworkRowsWindow, thumbnailWidth } from "../lib/artwork-window";
  let {
    games,
    groups = [],
    coverSize,
    compact = true,
    initialScroll = 0,
    onscroll,
    onopen,
    onfavorite,
    onhide,
    onlaunch,
    onsavegroup,
    oncontext,
    versionCount,
  }: {
    games: Game[];
    groups?: LibraryGroup[];
    coverSize: number;
    compact?: boolean;
    initialScroll?: number;
    onscroll: (top: number) => void;
    onopen: (g: Game) => void;
    onfavorite: (g: Game) => void;
    onhide?: (g: Game) => void;
    onlaunch: (g: Game) => void;
    onsavegroup?: (id: string) => void;
    oncontext?: (event: MouseEvent, game: Game) => void;
    versionCount?: (game: Game) => number;
  } = $props();
  let width = $state(900),
    height = $state(700),
    top = $state(0),
    viewport = $state<HTMLDivElement>();
  let gap = $derived(compact ? 16 : 24);
  let minimumWidth = $derived(Math.round(coverSize * (compact ? 0.75 : 1)));
  const artworkOwner = {};
  let direction = $state(1);
  let dpr = $state(1);
  let columns = $derived(
    Math.max(1, Math.floor((width + gap) / (minimumWidth + gap))),
  );
  let thumbnail = $derived(
    thumbnailWidth((width - gap * (columns - 1)) / columns, dpr),
  );
  let rowHeight = $derived(
    ((width - gap * (columns - 1)) / columns) * 1.5 + (compact ? 60 : 78),
  );
  let layout = $derived(gridLayout(games, groups, columns, rowHeight));
  let first = $derived(gridRowAt(layout.rows, top));
  let last = $derived(
    Math.min(layout.rows.length, gridRowAt(layout.rows, top + height) + 1),
  );
  let start = $derived(Math.max(0, first - 1));
  let end = $derived(Math.min(layout.rows.length, last + 1));
  let visible = $derived(layout.rows.slice(start, end));
  $effect(() => {
    setArtworkWindow(
      artworkOwner,
      artworkRowsWindow(layout.rows, first, last, direction, thumbnail),
    );
  });
  $effect(() => () => clearArtworkWindow(artworkOwner));
  $effect(() => {
    if (!viewport) return;
    void layout.height;
    void rowHeight;
    const measuredWidth = width,
      measuredHeight = height,
      requested = initialScroll;
    let live = true;
    // Resize bindings may update before the spacer's DOM style. Clamp only
    // after both actual measurements and the resulting layout have settled.
    void tick().then(() => {
      if (
        !live ||
        !viewport ||
        measuredWidth !== viewport.clientWidth ||
        measuredHeight !== viewport.clientHeight
      )
        return;
      const maximum = Math.max(
        0,
        viewport.scrollHeight - viewport.clientHeight,
      );
      viewport.scrollTop = Math.min(Math.max(0, requested), maximum);
      top = viewport.scrollTop;
    });
    return () => {
      live = false;
    };
  });
</script>

<svelte:window bind:devicePixelRatio={dpr} />

<div
  class="library-viewport"
  bind:this={viewport!}
  bind:clientWidth={width}
  bind:clientHeight={height}
  onscroll={() => {
    if (!viewport) return;
    direction = viewport.scrollTop >= top ? 1 : -1;
    top = viewport.scrollTop;
    onscroll(top);
  }}
>
  <div style:height={`${layout.height}px`} class="virtual-space">
    {#each visible as row (row.key)}
      {#if row.games}
        <div
          class="game-grid"
          style:grid-template-columns={`repeat(${columns},minmax(0,1fr))`}
          style:grid-auto-rows={`${rowHeight - gap}px`}
          style:column-gap={`${gap}px`}
          data-row-pitch={rowHeight}
          style:transform={`translateY(${row.top}px)`}
        >
          {#each row.games as game (game.id)}
            <GameCard
              {game}
              {thumbnail}
              priority={row.top + row.height > top && row.top < top + height
                ? 0
                : 1}
              {onopen}
              {onfavorite}
              {onhide}
              {onlaunch}
              {oncontext}
              versionCount={versionCount?.(game) ?? game.copies.length}
            />
          {/each}
        </div>
      {:else}
        <h2
          class="library-group-heading"
          aria-label={`${row.name} ${row.count}`}
          style:transform={`translateY(${row.top}px)`}
        >
          <span>{row.name}</span><span class="group-count">{row.count}</span>
          {#if onsavegroup}<button
              class="text-button save-group-collection"
              aria-label={`Save ${row.name} as Collection`}
              onclick={() => onsavegroup?.(row.groupId!)}
              ><FolderPlus size={14} />Save as Collection</button
            >{/if}
        </h2>
      {/if}
    {/each}
  </div>
</div>
