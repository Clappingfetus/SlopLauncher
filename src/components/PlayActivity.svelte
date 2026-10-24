<script lang="ts">
  import type { Game } from "../lib/types";
  import { activity } from "../lib/play-activity";
  import { RefreshCw } from "@lucide/svelte";
  let {
    game,
    refreshing = false,
    onrefresh,
  }: { game: Game; refreshing?: boolean; onrefresh?: () => void } = $props();
  let summary = $derived(activity(game));
  let hasSteam = $derived(
    game.copies.some((c) => c.source === "steam") || !!summary?.steam,
  );
  const date = (at?: number) =>
    at
      ? new Intl.DateTimeFormat(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        }).format(at)
      : "Not recorded";
  const hours = (minutes?: number) =>
    minutes !== undefined
      ? `${new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 }).format(minutes / 60)} h`
      : "Not available";
</script>

<section class="play-activity" aria-label="Play activity">
  <h2>Play activity</h2>
  <div class="activity-cards">
    <div class="activity-card">
      <h3>SlopLauncher</h3>
      <dl>
        <div>
          <dt>Last launcher activity</dt>
          <dd>{date(summary?.launcherLastAt)}</dd>
        </div>
        <div>
          <dt>Launches from this app</dt>
          <dd>{summary?.launcherLaunchCount ?? 0}</dd>
        </div>
      </dl>
      <p>Records successful launch requests from this PC.</p>
    </div>
    {#if hasSteam}<div class="activity-card">
        <h3>
          Steam {#if onrefresh}<button
              class="text-button"
              disabled={refreshing}
              onclick={onrefresh}
              ><RefreshCw
                size={12}
                class={refreshing ? "spin" : ""}
              />{refreshing ? "Refreshing…" : "Refresh"}</button
            >{/if}
        </h3>
        <dl>
          <div>
            <dt>Last played on Steam</dt>
            <dd>
              {summary?.steam?.lastPlayedAt
                ? date(summary.steam.lastPlayedAt)
                : summary?.steam?.recentMinutes
                  ? "Within the last two weeks"
                  : "Not reported"}
            </dd>
          </div>
          <div>
            <dt>Total Steam playtime</dt>
            <dd>{hours(summary?.steam?.totalMinutes)}</dd>
          </div>
          <div
            title={summary?.steam?.recentCheckedAt
              ? `Steam two-week snapshot updated ${date(summary.steam.recentCheckedAt)}`
              : "Steam has not returned recent playtime"}
          >
            <dt>Steam · last two weeks</dt>
            <dd>{hours(summary?.steam?.recentMinutes)}</dd>
          </div>
        </dl>
        <p>
          {summary?.steam
            ? `Steam snapshot: ${date(summary.steam.checkedAt)}`
            : "Connect Steam and make your game details visible to import activity."}
        </p>
      </div>{/if}
  </div>
</section>

<style>
  .play-activity {
    margin: 26px 0;
  }
  .play-activity h2 {
    margin-bottom: 14px;
  }
  .activity-cards {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(240px, 1fr));
    gap: 12px;
  }
  .activity-card {
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 16px;
    background: var(--surface);
  }
  .activity-card h3 {
    display: flex;
    align-items: center;
    justify-content: space-between;
    font-size: 13px;
    margin: 0 0 12px;
  }
  .activity-card dl {
    margin: 0;
    font-size: 12px;
  }
  .activity-card dl > div {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 5px 0;
  }
  .activity-card dt {
    color: var(--muted);
  }
  .activity-card dd {
    margin: 0;
    text-align: right;
  }
  .activity-card p {
    font-size: 10px;
    color: var(--faint);
    margin: 12px 0 0;
    line-height: 1.5;
  }
  .activity-card .text-button {
    font-size: 10px;
  }
</style>
