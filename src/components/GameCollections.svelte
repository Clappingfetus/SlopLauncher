<script lang="ts">
  import { FolderPlus } from "@lucide/svelte";
  import type { GameCollection } from "../lib/types";
  let {
    gameId,
    collections,
    disabled = false,
    onchange,
    oncreate,
  }: {
    gameId: string;
    collections: GameCollection[];
    disabled?: boolean;
    onchange: (id: string, included: boolean) => void;
    oncreate: () => void;
  } = $props();
</script>

<section class="game-collections" aria-label="Game collections">
  <h2>
    Collections<button class="text-button" {disabled} onclick={oncreate}
      ><FolderPlus size={14} />New collection</button
    >
  </h2>
  {#if disabled}<p class="field-hint">
      Collections are available in your own library.
    </p>
  {:else if collections.length}<div class="game-collection-choices">
      {#each collections as collection (collection.id)}<label
          class="collection-check"
          ><input
            type="checkbox"
            aria-label={`Add to ${collection.name}`}
            checked={collection.gameIds.includes(gameId)}
            disabled={!!collection.rule}
            title={collection.rule
              ? "Membership follows saved group rules"
              : undefined}
            onchange={(e) => onchange(collection.id, e.currentTarget.checked)}
          /><span>{collection.name}{collection.rule ? " · Live" : ""}</span
          ></label
        >{/each}
    </div>
  {:else}<p class="field-hint">
      Group games into your own folders. A game can belong to several
      collections.
    </p>{/if}
</section>
