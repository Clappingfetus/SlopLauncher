<script lang="ts">
  import { Heart, Play, Download, Eye, EyeOff } from "@lucide/svelte";
  import { consoleName } from "../lib/emulation";
  import { sourceNames, type Game } from "../lib/types";
  import { preferredCopy } from "../lib/library";
  import { launchAction } from "../lib/launch-action";
  import Cover from "./Cover.svelte";
  let {
    game,
    thumbnail,
    priority,
    onopen,
    onfavorite,
    onhide,
    onlaunch,
    oncontext,
    versionCount = game.copies.length,
  }: {
    game: Game;
    thumbnail: number;
    priority: number;
    onopen: (game: Game) => void;
    onfavorite: (game: Game) => void;
    onhide?: (game: Game) => void;
    onlaunch: (game: Game) => void;
    oncontext?: (event: MouseEvent, game: Game) => void;
    versionCount?: number;
  } = $props();
  let copy = $derived(preferredCopy(game));
  let installed = $derived(!!copy?.installed);
  let action = $derived(launchAction(game));
</script>

<article class="game-card" class:game-hidden={game.hidden}>
  <div class="cover-wrap">
    <button
      class="cover-button"
      oncontextmenu={(event) => oncontext?.(event, game)}
      onclick={() => onopen(game)}
      aria-label={`View ${game.title}`}
      ><Cover {game} {thumbnail} {priority} /></button
    >
    {#if onhide}<button
        class="card-hide"
        class:active={game.hidden}
        aria-label={`${game.hidden ? "Unhide" : "Hide"} ${game.title}`}
        title={game.hidden ? "Unhide game" : "Hide game"}
        aria-pressed={!!game.hidden}
        onclick={() => onhide?.(game)}
        >{#if game.hidden}<EyeOff size={13} />{:else}<Eye
            size={13}
          />{/if}</button
      >{/if}
    <button
      class:active={game.favorite}
      class="card-favorite"
      aria-label={`${game.favorite ? "Unfavorite" : "Favorite"} ${game.title}`}
      aria-pressed={game.favorite}
      onclick={() => onfavorite(game)}
      ><Heart
        size={15}
        fill={game.favorite ? "currentColor" : "none"}
      /></button
    >
    <button
      class="card-play"
      disabled={!!action.unavailable}
      title={action.unavailable || action.label}
      aria-label={`${installed ? "Play" : action.storeUrl ? action.label : "Install"} ${game.title}`}
      onclick={() => onlaunch(game)}
      >{#if installed}<Play size={17} fill="currentColor" />{:else}<Download
          size={17}
        />{/if}</button
    >
  </div>
  <button class="game-title" title={game.title} onclick={() => onopen(game)}
    >{game.title}</button
  >
  <div class="game-caption">
    <span class="source-dot" data-source={copy?.source}></span><span
      >{copy?.emulation
        ? consoleName(copy?.emulation?.platform)
        : (sourceNames[copy?.source] ?? "Local")}{versionCount > 1
        ? ` +${versionCount - 1}`
        : ""}</span
    >{#if installed}<span class="installed-indicator" title="Installed"
        >Installed</span
      >{/if}
  </div>
</article>
