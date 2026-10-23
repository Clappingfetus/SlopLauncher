<script lang="ts">
  import { Trophy, LockKeyhole, Check, LoaderCircle } from "@lucide/svelte";
  import RetroAchievementLink from "./RetroAchievementLink.svelte";
  import type { Game } from "../lib/types";
  import { preferredCopy } from "../lib/library";
  import { currentResult } from "../lib/enrichment";
  let {
    game,
    provider = "achievements",
    onmatch = () => {},
    revision,
    connected,
    enabled,
    busy,
    onconnect,
  }: {
    game: Game;
    provider?: "achievements" | "retroachievements";
    onmatch?: (id: number | undefined) => void;
    revision: number;
    connected: boolean;
    enabled: boolean;
    busy: boolean;
    onconnect: () => void;
  } = $props();
  let filter = $state("all"),
    limit = $state(8),
    revealed = $state<string[]>([]);
  let retro = $derived(provider === "retroachievements");
  let sourceName = $derived(retro ? "RetroAchievements" : "Steam");
  let mode = $state("all");
  let supported = $derived(
    preferredCopy(game)?.source === (retro ? "emulated" : "steam"),
  );
  let record = $derived(
    connected ? currentResult(game, provider, revision) : undefined,
  );
  let data = $derived(record?.data?.achievements);
  let items = $derived(
    (data?.items ?? []).map((a) =>
      mode === "hardcore"
        ? {
            ...a,
            unlocked: a.hardcoreUnlocked,
            unlockedAt: a.hardcoreUnlockedAt,
          }
        : a,
    ),
  );
  let unlocked = $derived(items.filter((a) => a.unlocked).length);
  let filtered = $derived(
    items
      .filter(
        (a) =>
          filter === "all" ||
          (filter === "unlocked" ? a.unlocked === true : a.unlocked === false),
      )
      .sort(
        (a, b) =>
          Number(!!b.unlocked) - Number(!!a.unlocked) ||
          (b.unlockedAt ?? 0) - (a.unlockedAt ?? 0) ||
          a.id.localeCompare(b.id),
      ),
  );
</script>

<section class="achievements-section" aria-label="Achievements">
  <div class="achievement-heading">
    <h2><Trophy size={17} />Achievements</h2>
    {#if supported}<span>{sourceName}</span>{/if}
  </div>
  {#if !supported}<p class="achievement-message">
      Achievement tracking is not available for this source yet.
    </p>
  {:else if !enabled}<p class="achievement-message">
      Connect {sourceName} in the desktop app to see your achievements.
    </p>
  {:else if !connected}<p class="achievement-message">
      Connect your {sourceName} account to see achievement progress.
    </p>
    <button class="text-button" onclick={onconnect}>Connect {sourceName}</button
    >
  {:else if data && items.length}
    {#if retro}<p class="achievement-message">
        {data.title} · {data.consoleName}
      </p>{/if}
    {#if data.progressAvailable}
      <div class="achievement-summary">
        <strong>{unlocked}<span> / {items.length} unlocked</span></strong><span
          >{Math.round((unlocked / items.length) * 100)}%</span
        >
      </div>
      <progress
        aria-label="Achievement completion"
        value={unlocked}
        max={items.length}
      ></progress>
    {:else}<p class="achievement-message">
        Your progress is unavailable. Check your {sourceName} connection{retro
          ? ""
          : " and profile privacy settings"}. Achievement definitions are shown
        below.
      </p>{/if}
    <div class="achievement-tools">
      <span
        >{#if busy}<LoaderCircle
            size={12}
            class="spin"
          />Updating…{:else}{record?.status === "unavailable"
            ? "Cached progress"
            : "Updated"}
          {new Date(record?.dataCheckedAt ?? record!.checkedAt).toLocaleString(
            undefined,
            {
              month: "short",
              day: "numeric",
              hour: "numeric",
              minute: "2-digit",
            },
          )}{/if}</span
      >
      {#if retro && data.progressAvailable}<select
          aria-label="Achievement mode"
          bind:value={mode}
          onchange={() => (limit = 8)}
          ><option value="all">All unlocks</option><option value="hardcore"
            >Hardcore</option
          ></select
        >{/if}
      {#if data.progressAvailable}<select
          aria-label="Filter achievements"
          bind:value={filter}
          onchange={() => (limit = 8)}
          ><option value="all">All achievements</option><option value="unlocked"
            >Unlocked</option
          ><option value="locked">Locked</option></select
        >{/if}
    </div>
    {#if record?.status === "unavailable"}<p class="achievement-message">
        {sourceName} could not be reached. Saved progress is shown; a retry is scheduled.
      </p>{/if}
    <ul class="achievement-list">
      {#each filtered.slice(0, limit) as achievement (achievement.id)}
        {@const secret =
          achievement.hidden &&
          !achievement.unlocked &&
          !revealed.includes(achievement.id)}
        <li class:unlocked={achievement.unlocked}>
          <span class="achievement-icon"
            >{#if achievement.unlocked}<Trophy
                size={19}
              />{:else if secret}<LockKeyhole size={18} />{:else}<Trophy
                size={19}
              />{/if}</span
          >
          <div class="achievement-copy">
            <strong>{secret ? "Secret achievement" : achievement.name}</strong>
            {#if retro && achievement.points !== undefined}<span
                class="achievement-message">{achievement.points} points</span
              >{/if}
            {#if secret}<button
                class="text-button"
                onclick={() => (revealed = [...revealed, achievement.id])}
                >Reveal details</button
              >{:else if achievement.description}<p>
                {achievement.description}
              </p>{/if}
            {#if achievement.unlockedAt}<time
                datetime={new Date(achievement.unlockedAt).toISOString()}
                >Unlocked {new Date(
                  achievement.unlockedAt,
                ).toLocaleDateString()}</time
              >{/if}
          </div>
          <span class="achievement-state"
            >{#if achievement.unlocked}<Check size={14} /><span>Unlocked</span
              >{:else}<span
                >{data.progressAvailable ? "Locked" : "Unknown"}</span
              >{/if}</span
          >
        </li>
      {/each}
    </ul>
    {#if !filtered.length}<p class="achievement-message">
        No {filter} achievements.
      </p>{/if}
    {#if filtered.length > limit}<button
        class="text-button achievement-more"
        onclick={() => (limit += 20)}
        >Show more · {filtered.length - limit} remaining</button
      >{/if}
  {:else if data}<p class="achievement-message">
      {sourceName} lists no achievements for this game.
    </p>
  {:else if record?.status === "unavailable" || record?.status === "needs-match" || record?.status === "needs-key"}<p
      class="achievement-message"
    >
      {record.message ??
        `${sourceName} achievements are temporarily unavailable. A retry is scheduled.`}
    </p>
  {:else}<p class="achievement-message achievement-loading">
      <LoaderCircle size={14} class="spin" />Loading {sourceName} achievements…
    </p>{/if}
  {#if retro && enabled && connected}
    <RetroAchievementLink
      gameId={preferredCopy(game)?.emulation?.retroAchievementsGameId}
      onchange={onmatch}
    />
    <p class="achievement-message">
      Unlocks are earned in a RetroAchievements-compatible emulator. Enable its
      RetroAchievements integration and use a supported game file.
    </p>
  {/if}
</section>

<style>
  .achievements-section {
    margin: 28px 0;
    padding-top: 24px;
    border-top: 1px solid var(--line);
  }
  .achievement-heading,
  .achievement-summary,
  .achievement-tools {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  .achievement-heading h2 {
    display: flex;
    align-items: center;
    gap: 9px;
    margin: 0;
  }
  .achievement-heading > span,
  .achievement-message,
  .achievement-summary span,
  .achievement-tools,
  .achievement-copy p,
  .achievement-copy time,
  .achievement-state {
    color: var(--muted);
    font-size: 12px;
  }
  .achievement-heading > span {
    border: 1px solid var(--line);
    padding: 3px 8px;
    border-radius: 5px;
  }
  .achievement-summary {
    margin-top: 18px;
  }
  .achievement-summary strong {
    font-size: 20px;
    font-weight: 500;
  }
  progress {
    display: block;
    width: 100%;
    height: 5px;
    margin: 9px 0 16px;
    border: 0;
    border-radius: 3px;
    overflow: hidden;
    background: var(--surface);
  }
  progress::-webkit-progress-bar {
    background: var(--surface);
  }
  progress::-webkit-progress-value {
    background: var(--accent);
  }
  .achievement-tools {
    margin-top: 14px;
    flex-wrap: wrap;
  }
  .achievement-tools > span,
  .achievement-loading {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  select {
    padding: 6px 8px;
    font-size: 12px;
    max-width: 100%;
  }
  .achievement-list {
    list-style: none;
    margin: 12px 0 0;
    padding: 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .achievement-icon {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    flex-shrink: 0;
    background: var(--surface);
    border-radius: 9px;
    color: var(--muted);
  }
  .unlocked .achievement-icon,
  .unlocked .achievement-state {
    color: var(--accent);
  }
  .achievement-copy {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
  }
  .achievement-copy strong {
    display: block;
    font-weight: 500;
    font-size: 13px;
  }
  .achievement-copy p {
    margin: 4px 0 0;
    line-height: 1.5;
  }
  .achievement-copy time {
    display: block;
    margin-top: 5px;
    font-size: 11px;
  }
  .achievement-copy button {
    margin-top: 4px;
    font-size: 12px;
  }
  .achievement-state {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
  }
  .achievement-more {
    margin-top: 14px;
  }
  @media (max-width: 1000px) {
    .achievement-state > span {
      display: none;
    }
  }
</style>
