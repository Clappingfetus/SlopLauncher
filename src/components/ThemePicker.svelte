<script lang="ts">
  import { Check } from "@lucide/svelte";
  import { themes, resolveTheme, type ThemeId } from "../lib/themes";
  let {
    value,
    onchange,
  }: { value: ThemeId; onchange: (theme: ThemeId) => void } = $props();
</script>

<section class="theme-picker" aria-labelledby="theme-picker-title">
  <div class="theme-picker-heading">
    <div>
      <h3 id="theme-picker-title">Color theme</h3>
      <p>Make this space yours.</p>
    </div>
    <span>{resolveTheme(value).name}</span>
  </div>
  <div class="theme-grid" role="group" aria-label="Color themes">
    {#each themes as theme (theme.id)}
      <button
        type="button"
        class="theme-option"
        class:selected={value === theme.id}
        aria-label={`${theme.name} theme`}
        aria-pressed={value === theme.id}
        onclick={() => onchange(theme.id)}
      >
        <span
          class="theme-swatch"
          aria-hidden="true"
          style={`--swatch-bg:${theme.colors[0]};--swatch-sidebar:${theme.colors[1]};--swatch-surface:${theme.colors[2]};--swatch-accent:${theme.colors[8]}`}
        >
          <span class="swatch-sidebar"><i></i><i></i><i></i></span><span
            class="swatch-content"
            ><i></i><span><b></b><b></b><b></b></span></span
          >
        </span>
        <span class="theme-option-label"
          >{theme.name}{#if value === theme.id}<Check
              size={12}
              strokeWidth={2.2}
            />{/if}</span
        >
      </button>
    {/each}
  </div>
</section>

<style>
  .theme-picker {
    padding: 18px 0 22px;
    border-bottom: 1px solid var(--line);
  }
  .theme-picker-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 15px;
  }
  h3 {
    font-size: 13px;
    font-weight: 500;
    margin: 0;
  }
  p {
    font-size: 11px;
    color: var(--faint);
    margin-top: 5px;
  }
  .theme-picker-heading > span {
    font-size: 11px;
    color: var(--accent);
  }
  .theme-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }
  .theme-option {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding: 5px;
    gap: 0;
    border: 1px solid var(--line);
    border-radius: 8px;
    min-width: 0;
    transition:
      border-color 120ms,
      background 120ms;
  }
  .theme-option:hover {
    background: var(--surface);
    border-color: var(--muted);
  }
  .theme-option.selected {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 7%, transparent);
  }
  .theme-swatch {
    height: 46px;
    display: flex;
    border-radius: 4px;
    overflow: hidden;
    background: var(--swatch-bg);
    border: 1px solid #80808025;
  }
  .swatch-sidebar {
    display: flex;
    flex-direction: column;
    gap: 5px;
    width: 23%;
    padding: 8px 4px;
    background: var(--swatch-sidebar);
  }
  .swatch-sidebar i {
    height: 2px;
    border-radius: 1px;
    background: var(--swatch-accent);
    opacity: 0.4;
  }
  .swatch-sidebar i:first-child {
    height: 5px;
    opacity: 1;
    margin-bottom: 1px;
  }
  .swatch-content {
    flex: 1;
    padding: 8px 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .swatch-content > i {
    width: 50%;
    height: 3px;
    background: var(--swatch-accent);
    border-radius: 2px;
  }
  .swatch-content > span {
    display: flex;
    gap: 3px;
    height: 22px;
  }
  .swatch-content b {
    flex: 1;
    background: var(--swatch-surface);
    border-radius: 2px;
  }
  .swatch-content b:first-child {
    background: color-mix(
      in srgb,
      var(--swatch-accent) 30%,
      var(--swatch-surface)
    );
  }
  .theme-option-label {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 2px 3px;
    font-size: 11px;
  }
  .theme-option-label :global(svg) {
    color: var(--accent);
  }
  @media (max-width: 420px) {
    .theme-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .theme-option {
      transition: none;
    }
  }
</style>
