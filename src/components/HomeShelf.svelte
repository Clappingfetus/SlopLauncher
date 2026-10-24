<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { ChevronLeft, ChevronRight } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import GameCard from "./GameCard.svelte";
  import { setArtworkWindow, clearArtworkWindow } from "../lib/backend";
  import { thumbnailWidth } from "../lib/artwork-window";
  let {
    games,
    cardWidth = 132,
    active = false,
    draggable = true,
    onopen,
    onfavorite,
    onhide,
    onlaunch,
    oncontext,
    versionCount,
    initialScroll = 0,
    onscroll,
  }: {
    games: Game[];
    cardWidth?: number;
    active?: boolean;
    draggable?: boolean;
    onopen: (g: Game) => void;
    onfavorite: (g: Game) => void;
    onhide: (g: Game) => void;
    onlaunch: (g: Game) => void;
    oncontext: (event: MouseEvent, g: Game) => void;
    versionCount: (g: Game) => number;
    initialScroll?: number;
    onscroll?: (value: number) => void;
  } = $props();
  let track = $state<HTMLDivElement>(),
    width = $state(900),
    left = $state(untrack(() => initialScroll)),
    near = $state(untrack(() => active)),
    dpr = $state(1);
  const owner = {};
  let pitch = $derived(cardWidth + 16),
    first = $derived(Math.max(0, Math.floor(left / pitch) - 2)),
    last = $derived(
      Math.min(games.length, Math.ceil((left + width) / pitch) + 3),
    );
  let visible = $derived(near ? games.slice(first, last) : []);
  let totalWidth = $derived(Math.max(0, games.length * pitch - 16));
  let thumbnail = $derived(thumbnailWidth(cardWidth, dpr));
  onMount(() => {
    dpr = devicePixelRatio || 1;
    const observer = new IntersectionObserver(
      (entries) => (near = entries[0].isIntersecting),
      { root: track?.closest(".home-page"), rootMargin: "350px 0px" },
    );
    if (track) {
      observer.observe(track);
      track.scrollLeft = initialScroll;
      // A collection may shrink while details are open; use the browser's
      // clamped position so a stale offset cannot leave a short shelf blank.
      left = track.scrollLeft;
    }
    return () => observer.disconnect();
  });
  $effect(() => {
    if (!near) {
      clearArtworkWindow(owner);
      return;
    }
    // This decode window stays small; persistent cover preparation still uses
    // the full library independently of which shelf or page is visible.
    setArtworkWindow(
      owner,
      games.slice(first, Math.min(games.length, last + 5)).flatMap((g, i) =>
        g.cover
          ? [
              {
                url: g.cover,
                thumbnail: i < last - first ? thumbnail : 256,
                priority: i < last - first ? 0 : 2,
              },
            ]
          : [],
      ),
    );
  });
  $effect(() => () => clearArtworkWindow(owner));
  function scroll(direction: number) {
    track?.scrollBy({
      left: direction * Math.max(pitch, width - pitch),
      behavior: matchMedia("(prefers-reduced-motion: reduce)").matches
        ? "instant"
        : "smooth",
    });
  }
</script>

<div class="shelf-scroller">
  <button
    class="shelf-arrow prev"
    aria-label="Scroll shelf left"
    disabled={left < 2}
    onclick={() => scroll(-1)}><ChevronLeft size={17} /></button
  >
  <div
    class="shelf-track"
    bind:this={track}
    bind:clientWidth={width}
    onscroll={(event) => {
      left = event.currentTarget.scrollLeft;
      onscroll?.(left);
    }}
    aria-label="Scroll games horizontally"
  >
    <div
      class="shelf-space"
      style:width={`${totalWidth}px`}
      style:height={`${cardWidth * 1.5 + 52}px`}
    >
      {#each visible as game, i (game.id)}<div
          class="shelf-card"
          style:width={`${cardWidth}px`}
          style:left={`${(first + i) * pitch}px`}
        >
          <GameCard
            {game}
            {thumbnail}
            priority={0}
            {draggable}
            {onopen}
            {onfavorite}
            {onhide}
            {onlaunch}
            {oncontext}
            versionCount={versionCount(game)}
          />
        </div>{/each}
    </div>
  </div>
  <button
    class="shelf-arrow next"
    aria-label="Scroll shelf right"
    disabled={left + width >= totalWidth - 2}
    onclick={() => scroll(1)}><ChevronRight size={17} /></button
  >
</div>

<style>
  .shelf-scroller {
    position: relative;
  }
  .shelf-track {
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
    scrollbar-color: var(--line) transparent;
    outline-offset: 3px;
    padding: 4px 3px 8px;
    overscroll-behavior-inline: contain;
  }
  .shelf-space {
    position: relative;
  }
  .shelf-card {
    position: absolute;
    top: 0;
  }
  .shelf-arrow {
    position: absolute;
    top: calc(50% - 22px);
    z-index: 5;
    display: grid;
    place-items: center;
    width: 30px;
    height: 44px;
    border-radius: 7px;
    background: var(--surface);
    border: 1px solid var(--line);
    box-shadow: 0 3px 12px var(--shadow);
    opacity: 0;
    transition: opacity 0.12s;
  }
  .shelf-scroller:hover .shelf-arrow,
  .shelf-arrow:focus-visible {
    opacity: 1;
  }
  .shelf-arrow:disabled {
    visibility: hidden;
  }
  .prev {
    left: -12px;
  }
  .next {
    right: -12px;
  }
  @media (prefers-reduced-motion: reduce) {
    .shelf-arrow {
      transition: none;
    }
  }
</style>
