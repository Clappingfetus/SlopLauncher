<script lang="ts">
  import type { Game } from "../lib/types";
  import { Gamepad2 } from "@lucide/svelte";
  import {
    acquireArtwork,
    peekArtwork,
    retryArtwork,
    artworkCatalogState,
    invalidatePreparedArtwork,
  } from "../lib/backend";
  let {
    game,
    hero = false,
    thumbnail = 0,
    priority = 0,
  }: {
    game: Game;
    hero?: boolean;
    thumbnail?: number;
    priority?: number;
  } = $props();
  let failure = $state("");
  let resolved = $state({ key: "", src: "" });
  let reconnect = $state(0);
  let reveal = $state(false);
  let rejected = $state.raw<{ identity: string; urls: string[] }>({
    identity: "",
    urls: [],
  });
  let candidates = $derived([
    ...new Set(
      [
        hero ? game.hero : game.cover,
        ...(game.artworkFallbacks?.[hero ? "hero" : "cover"] ?? []),
      ].filter((url): url is string => !!url),
    ),
  ]);
  let candidateIdentity = $derived(JSON.stringify([game.id, candidates]));
  let url = $derived(
    candidates.find(
      (url) =>
        rejected.identity !== candidateIdentity || !rejected.urls.includes(url),
    ),
  );
  function rejectCurrent() {
    if (!url) return;
    rejected = {
      identity: candidateIdentity,
      urls: [
        ...(rejected.identity === candidateIdentity ? rejected.urls : []),
        url,
      ],
    };
  }
  let key = $derived(JSON.stringify([url, thumbnail]));
  let failed = $derived(failure === key);
  let display = $derived.by(() => {
    void $artworkCatalogState;
    return url
      ? resolved.key === key
        ? resolved.src
        : peekArtwork(url, thumbnail)
      : "";
  });
  $effect(() => {
    void reconnect;
    const current = url;
    const currentKey = key;
    let live = true;
    // Track the request identity, never carry a previous game's image across a rematch.
    if (!current) return;
    const alreadyVisible = !!peekArtwork(current, thumbnail);
    const request = acquireArtwork(current, thumbnail, priority);
    request.promise
      .then((value) => {
        if (live) {
          reveal = !alreadyVisible && !display;
          failure = "";
          resolved = { key: currentKey, src: value };
        }
      })
      .catch(() => {
        if (live) {
          failure = currentKey;
          rejectCurrent();
        }
      });
    return () => {
      live = false;
      request.release();
    };
  });
</script>

<svelte:window
  ononline={() => {
    for (const rejectedUrl of rejected.urls)
      retryArtwork(rejectedUrl, thumbnail);
    rejected = { identity: "", urls: [] };
    if (failed && url) {
      retryArtwork(url, thumbnail);
      failure = "";
      reconnect++;
    }
  }}
/>

{#if display && !failed}<img
    src={display}
    class:artwork-reveal={reveal && resolved.key === key}
    alt=""
    loading="eager"
    decoding="async"
    onerror={() => {
      failure = key;
      if (url) {
        invalidatePreparedArtwork(url);
        retryArtwork(url, thumbnail);
      }
      rejectCurrent();
    }}
  />{:else}<div class="cover-fallback" class:loading-art={url && !failed}>
    <Gamepad2 size={hero ? 48 : 28} strokeWidth={1} /><span>{game.title}</span>
  </div>{/if}
