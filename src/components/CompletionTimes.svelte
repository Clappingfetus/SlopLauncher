<script lang="ts">
  import { Clock3 } from "@lucide/svelte";
  import type { Game } from "../lib/types";
  import { currentResult, hours } from "../lib/enrichment";
  import { desktop } from "../lib/backend";
  let { game }: { game: Game } = $props();
  let times = $derived(currentResult(game, "hltb"));
  let estimates = $derived(times?.data);
</script>

<section class="completion-times" aria-label="HowLongToBeat estimates">
  <div class="completion-title">
    <Clock3 size={13} /><span>HowLongToBeat estimates</span>
  </div>
  {#if estimates && (estimates.mainHours || estimates.extraHours || estimates.completionistHours)}
    <dl>
      <div>
        <dt>Main story</dt>
        <dd>{hours(estimates.mainHours)}</dd>
      </div>
      <div>
        <dt>Main + extras</dt>
        <dd>{hours(estimates.extraHours)}</dd>
      </div>
      <div>
        <dt>Completionist</dt>
        <dd>{hours(estimates.completionistHours)}</dd>
      </div>
    </dl>
    <p>
      {times?.status === "unavailable"
        ? "Cached estimates"
        : "Community averages"} · HowLongToBeat
    </p>
  {:else}<p>
      {times?.status === "unavailable"
        ? "Estimates are temporarily unavailable. View them on HowLongToBeat."
        : times?.status === "not-found" || times?.status === "needs-match"
          ? "No verified estimates yet. Find community times on HowLongToBeat."
          : desktop
            ? "Looking for a matching entry…"
            : "Open HowLongToBeat to view community estimates."}
    </p>{/if}
</section>
