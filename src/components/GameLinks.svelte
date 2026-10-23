<script lang="ts">
  import {
    BookOpen,
    Clock3,
    MessagesSquare,
    BookMarked,
    Store,
    Puzzle,
    Trophy,
  } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import { resourceLinks } from "../lib/enrichment";
  import { openResource } from "../lib/backend";
  let {
    game,
    revision = 0,
    onerror = () => {},
  }: {
    game: Game;
    revision?: number;
    onerror?: (message: string) => void;
  } = $props();
  let links = $derived(resourceLinks(game, revision));
  let items = $derived(
    [
      { name: "PCGamingWiki", label: "Wiki", icon: BookOpen, url: links.pcgw },
      { name: "HowLongToBeat", label: "HLTB", icon: Clock3, url: links.hltb },
      {
        name: "Steam Discussions",
        label: "Discussions",
        icon: MessagesSquare,
        url: links.discussions,
      },
      {
        name: "Steam Guides",
        label: "Guides",
        icon: BookMarked,
        url: links.guides,
      },
      {
        name: links.storeName ?? "Store",
        label: links.storeLabel ?? "Store",
        icon: Store,
        url: links.store,
      },
      { name: "Nexus Mods", label: "Mods", icon: Puzzle, url: links.nexus },
      {
        name: "RetroAchievements",
        label: "Achievements",
        icon: Trophy,
        url: links.retroAchievements,
      },
    ].filter((item) => item.url),
  );
  function open(event: MouseEvent, url: string) {
    event.preventDefault();
    void openResource(url).catch((error) => onerror(String(error)));
  }
</script>

<nav class="game-links" aria-label="Game links">
  {#each items as item (item.name)}
    <a
      class="game-link-button"
      href={item.url}
      target="_blank"
      rel="noopener noreferrer"
      title={item.name}
      aria-label={item.name}
      onclick={(event) => open(event, item.url!)}
    >
      <item.icon size={15} strokeWidth={1.65} /><span>{item.label}</span>
    </a>
  {/each}
</nav>
