<script lang="ts">
  import { setupServices, type SetupId } from "../lib/setup";
  let {
    service,
    expanded = false,
    compact = false,
  }: { service: SetupId; expanded?: boolean; compact?: boolean } = $props();
  const helpId = $props.id();
  let help = $derived(setupServices.find((s) => s.id === service)!);
</script>

<details class="connection-help" class:compact open={expanded}>
  <summary title={help.tip} aria-describedby={helpId}
    >How to connect {help.name}</summary
  >
  <ol>
    {#each help.steps as step}<li>{step}</li>{/each}
  </ol>
  <p id={helpId}>{help.tip}</p>
</details>

<style>
  .connection-help {
    margin: 12px 0;
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 11px 13px;
    font-size: 12px;
    line-height: 1.65;
  }
  .connection-help.compact {
    margin: 8px 0;
    padding: 7px 10px;
    line-height: 1.45;
    font-size: 11px;
  }
  summary {
    cursor: pointer;
    color: var(--text);
    font-weight: 500;
  }
  summary:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 4px;
  }
  ol {
    padding-left: 19px;
    margin: 10px 0;
  }
  li + li {
    margin-top: 8px;
  }
  p {
    color: var(--faint);
    margin: 8px 0 0;
  }
</style>
