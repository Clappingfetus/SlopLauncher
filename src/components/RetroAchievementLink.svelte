<script lang="ts">
  import { retroGameId } from "../lib/retroachievements";
  let {
    gameId,
    onchange,
  }: { gameId?: number; onchange: (id: number | undefined) => void } = $props();
  let value = $state(""),
    error = $state("");
  function save() {
    const id = retroGameId(value);
    if (!id) {
      error = "Enter a RetroAchievements game page URL or a positive game ID.";
      return;
    }
    error = "";
    onchange(id);
    value = "";
  }
</script>

<details class="retro-match" open={!gameId}>
  <summary
    >{gameId
      ? `Linked achievement set · #${gameId}`
      : "Link a RetroAchievements game"}</summary
  >
  <p>
    Paste the game page for this console. The launcher checks its console and
    shows the linked title. ROM compatibility is verified by your emulator.
  </p>
  <form
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <label
      >Game page or ID<input
        aria-label="RetroAchievements game page or ID"
        bind:value
        placeholder="https://retroachievements.org/game/…"
      /></label
    >
    <button class="secondary-button" type="submit">Link achievement set</button>
    {#if gameId}<button
        class="text-button"
        type="button"
        onclick={() => onchange(undefined)}>Unlink achievement set</button
      >{/if}
  </form>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
</details>

<style>
  .retro-match {
    margin: 14px 0;
    padding: 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  summary {
    cursor: pointer;
    font-size: 12px;
    color: var(--text);
  }
  p {
    font-size: 12px;
    color: var(--muted);
    line-height: 1.6;
  }
  form {
    display: flex;
    align-items: end;
    gap: 10px;
    flex-wrap: wrap;
  }
  label {
    flex: 1;
    min-width: 160px;
    font-size: 12px;
    color: var(--muted);
  }
  input {
    display: block;
    width: 100%;
    margin-top: 6px;
  }
</style>
