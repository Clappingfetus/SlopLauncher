<script lang="ts">
  import { Star, ArrowUpRight } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import { consensus, ratingRows, reviewNames } from "../lib/ratings";
  import { openResource } from "../lib/backend";
  let { game, onmatch }: { game: Game; onmatch: () => void } = $props();
  let rows = $derived(game.reviewSummary ?? ratingRows([game]));
  let combined = $derived(consensus(rows));
  let error = $state("");
  async function open(url: string) {
    try {
      await openResource(url);
    } catch {
      error = "Could not open the review page.";
    }
  }
</script>

<section class="reviews-panel" aria-label="Review ratings">
  <div class="reviews-heading">
    <h2>Review ratings</h2>
    <button class="text-button" onclick={onmatch}
      >Match review pages <ArrowUpRight size={12} /></button
    >
  </div>
  <div class="consensus">
    <div>
      <span class="eyebrow">CONSENSUS</span>
      <div class="score">{combined?.stars ?? "—"}<small>/ 10</small></div>
    </div>
    <div class="consensus-summary">
      <div
        class="stars"
        role="img"
        aria-label={combined
          ? `${combined.stars} out of 10 stars`
          : "No consensus rating yet"}
      >
        {#each Array.from({ length: 10 }, (_, i) => i) as i}<span class="star"
            ><Star size={16} /><span
              class="star-fill"
              style:width={`${Math.max(0, Math.min(1, (combined?.stars ?? 0) - i)) * 100}%`}
              ><Star size={16} fill="currentColor" /></span
            ></span
          >{/each}
      </div>
      <p>
        {combined
          ? `${combined.coverage} · ${combined.sources} of 5 sources`
          : "Ratings load in the background. Unavailable sources do not lower the score."}
      </p>
    </div>
  </div>
  <div class="review-sources">
    {#each rows as row}
      <div class="review-source">
        <div class="source-heading">
          <strong>{reviewNames[row.source]}</strong><span class="review-kind"
            >{row.source === "opencritic" ? "Critics" : "Users"}</span
          >
        </div>
        {#if row.rating}
          <button
            class="review-value"
            disabled={row.source === "steam" &&
              !game.copies.some((c) => c.source === "steam")}
            onclick={() => open(row.rating!.url)}
            title={`Open ${reviewNames[row.source]} reviews`}
          >
            {row.rating.value.toFixed(row.rating.scale === 100 ? 0 : 1)}<small
              >{row.source === "steam"
                ? "% positive"
                : ` / ${row.rating.scale}`}</small
            ><ArrowUpRight size={12} />
          </button>
          <p>
            {row.rating.count.toLocaleString()} ratings · {row.rating.platform}
          </p>
          <p
            title={row.checkedAt
              ? new Date(row.checkedAt).toLocaleString()
              : undefined}
          >
            {row.stale
              ? "Saved score · refresh unavailable"
              : `Updated ${row.checkedAt ? new Date(row.checkedAt).toLocaleDateString() : "recently"}`}
          </p>
        {:else}
          <div class="review-value">—</div>
          <p title={row.message}>
            {row.status === "unavailable"
              ? "Temporarily unavailable"
              : row.status === "not-found"
                ? "No published rating"
                : row.status === "needs-match"
                  ? "No verified match"
                  : "Not loaded yet"}
          </p>
        {/if}
      </div>
    {/each}
  </div>
  <details class="rating-method">
    <summary>How the consensus works</summary>
    <p>
      User ratings from Steam, GOG, Metacritic and itch.io are converted to a
      10-point scale and averaged equally, then weighted at 60%. OpenCritic’s
      critic average contributes 40%. Steam represents the percentage of
      positive reviews, not an average star rating. If one group is missing, the
      available group receives full weight. Unrated games stay last when
      sorting. Stars round to the nearest half, with a minimum of one star;
      sorting uses the precise consensus score. Sources refresh weekly when
      viewed or used for sorting. Saved scores remain available if a service
      fails.
    </p>
  </details>
  {#if error}<p role="alert">{error}</p>{/if}
</section>

<style>
  .reviews-panel {
    border: 1px solid var(--line);
    border-radius: 10px;
    padding: 20px;
    margin-bottom: 28px;
    background: var(--surface);
  }
  .reviews-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .reviews-heading h2 {
    margin: 0;
    font-size: 14px;
  }
  .consensus {
    display: flex;
    align-items: center;
    gap: 25px;
    padding: 21px 0;
  }
  .eyebrow {
    font-size: 9px;
    letter-spacing: 0.14em;
    opacity: 0.65;
  }
  .score {
    font-size: 34px;
    line-height: 1.3;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }
  .score small {
    font-size: 13px;
    font-weight: 400;
    margin-left: 7px;
    opacity: 0.55;
  }
  .stars {
    display: flex;
    gap: 3px;
    color: var(--accent);
  }
  .star {
    position: relative;
    display: block;
    width: 16px;
    height: 16px;
  }
  .star > :global(svg) {
    opacity: 0.25;
  }
  .star-fill {
    position: absolute;
    left: 0;
    top: 0;
    overflow: hidden;
    display: block;
    height: 16px;
  }
  .star-fill :global(svg) {
    max-width: none;
    display: block;
  }
  .consensus-summary p,
  .review-source p {
    font-size: 10px;
    opacity: 0.65;
    line-height: 1.6;
    margin: 7px 0 0;
  }
  .review-sources {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(135px, 1fr));
    gap: 10px;
  }
  .review-source {
    border-top: 1px solid var(--line);
    padding: 13px 0 4px;
    min-width: 0;
  }
  .source-heading {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .source-heading strong {
    font-size: 12px;
  }
  .review-kind {
    font-size: 9px;
    opacity: 0.6;
  }
  .review-value {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 21px;
    font-variant-numeric: tabular-nums;
    padding: 0;
    margin-top: 10px;
    border: 0;
    background: none;
    color: inherit;
  }
  .review-value:disabled {
    opacity: 1;
    cursor: default;
  }
  .review-value:disabled :global(svg) {
    display: none;
  }
  .review-value small {
    font-size: 10px;
    opacity: 0.65;
  }
  .review-value :global(svg) {
    opacity: 0.5;
    margin-left: 3px;
  }
  .rating-method {
    margin-top: 16px;
    font-size: 11px;
    opacity: 0.7;
  }
  .rating-method summary {
    cursor: pointer;
  }
  .rating-method p {
    line-height: 1.7;
    max-width: 80ch;
  }
</style>
