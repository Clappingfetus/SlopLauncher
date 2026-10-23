<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  import {
    MoreHorizontal,
    Info,
    FolderOpen,
    Gamepad2,
    Image,
    Trash2,
  } from "@lucide/svelte";
  let {
    gameId,
    copyId,
    browseUnavailable,
    ondetails,
    onbrowse,
    onemulation,
    onartwork,
    onuninstall,
    uninstallUnavailable = "",
    anchor,
    artworkOnly,
    onresetartwork,
    onclose,
  }: {
    gameId: string;
    copyId?: string;
    browseUnavailable: string;
    ondetails: () => void;
    onbrowse: () => void;
    onemulation?: () => void;
    onartwork?: () => void;
    onuninstall?: () => void;
    uninstallUnavailable?: string;
    anchor?: { x: number; y: number; target?: HTMLElement };
    artworkOnly?: "cover" | "hero";
    onresetartwork?: () => void;
    onclose?: () => void;
  } = $props();
  let trigger = $state<HTMLButtonElement>();
  let menu = $state<HTMLDivElement>();
  let expanded = $state(false);
  let openedIdentity = $state("");
  let anchorTop = 0,
    anchorLeft = 0;
  const menuId = $props.id();
  function dismiss(restoreFocus = false) {
    if (menu?.matches(":popover-open")) menu.hidePopover();
    if (restoreFocus)
      (anchor?.target ?? trigger)?.focus({ preventScroll: true });
  }
  async function show(last = false) {
    if (!menu?.isConnected) return;
    openedIdentity = JSON.stringify([gameId, copyId]);
    const rect = anchor
      ? { top: anchor.y, bottom: anchor.y, left: anchor.x, right: anchor.x }
      : trigger!.getBoundingClientRect();
    anchorTop = rect.top;
    anchorLeft = rect.left;
    menu.style.left = `${Math.max(12, Math.min(anchor ? rect.left : rect.right - 236, innerWidth - 248))}px`;
    menu.style.top = `${Math.max(12, rect.bottom + 4)}px`;
    if (!menu.matches(":popover-open")) menu.showPopover();
    const height = menu.getBoundingClientRect().height;
    menu.style.top = `${Math.max(12, Math.min(rect.bottom + 4, innerHeight - height - 12))}px`;
    await tick();
    const items = menu.querySelectorAll<HTMLButtonElement>(
      "button:not(:disabled)",
    );
    items[last ? items.length - 1 : 0]?.focus({ preventScroll: true });
  }
  function keyboard(event: KeyboardEvent) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      dismiss(true);
    } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
      event.preventDefault();
      const items = Array.from(
        menu!.querySelectorAll<HTMLButtonElement>("button:not(:disabled)"),
      );
      const index = items.indexOf(document.activeElement as HTMLButtonElement);
      const next =
        event.key === "Home"
          ? 0
          : event.key === "End"
            ? items.length - 1
            : (index + (event.key === "ArrowDown" ? 1 : -1) + items.length) %
              items.length;
      items[next]?.focus({ preventScroll: true });
    } else if (event.key === "Tab") dismiss(true);
  }
  $effect(() => {
    void gameId;
    void copyId;
    void anchor;
    untrack(() => {
      if (anchor) void show();
      else if (openedIdentity !== JSON.stringify([gameId, copyId])) dismiss();
    });
  });
  onMount(() => {
    const close = () => dismiss();
    const scroll = () => {
      if (anchor) {
        dismiss();
        return;
      }
      if (!trigger) return;
      const rect = trigger.getBoundingClientRect();
      if (
        Math.abs(rect.top - anchorTop) > 1 ||
        Math.abs(rect.left - anchorLeft) > 1
      )
        dismiss();
    };
    document.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", close);
    return () => {
      document.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", close);
    };
  });
</script>

{#if !anchor}<button
    bind:this={trigger}
    class="icon-button detail-more"
    aria-label="Game menu"
    title="Game menu"
    aria-haspopup="menu"
    aria-expanded={expanded}
    aria-controls={menuId}
    onclick={() => (expanded ? dismiss() : void show())}
    onkeydown={(event) => {
      if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        event.preventDefault();
        void show(event.key === "ArrowUp");
      }
    }}><MoreHorizontal size={20} /></button
  >{/if}
<div
  bind:this={menu}
  id={menuId}
  class="game-menu"
  data-game-menu
  popover="auto"
  role="menu"
  aria-label="Game actions"
  tabindex="-1"
  ontoggle={(event) => {
    expanded = event.newState === "open";
    if (!expanded) onclose?.();
  }}
  onkeydown={keyboard}
>
  {#if artworkOnly}
    <button
      role="menuitem"
      onclick={() => {
        dismiss();
        onartwork?.();
      }}
      ><Image size={16} />{artworkOnly === "cover"
        ? "Change Box Art"
        : "Change Background"}</button
    >
    <button
      role="menuitem"
      onclick={() => {
        dismiss();
        onresetartwork?.();
      }}
      ><Image size={16} />{artworkOnly === "cover"
        ? "Use Default Box Art"
        : "Use Default Background"}</button
    >
  {:else}
    <button
      role="menuitem"
      onclick={() => {
        dismiss();
        ondetails();
      }}><Info size={16} />Game Details</button
    >
    <button
      role="menuitem"
      disabled={!!browseUnavailable}
      title={browseUnavailable ||
        "Open this copy’s installation folder in Explorer"}
      onclick={() => {
        dismiss(true);
        onbrowse();
      }}><FolderOpen size={16} />Browse Game Files</button
    >
    {#if onartwork}<button
        role="menuitem"
        onclick={() => {
          dismiss();
          onartwork?.();
        }}><Image size={16} />Change Artwork</button
      >{/if}
    {#if onemulation}<button
        role="menuitem"
        onclick={() => {
          dismiss();
          onemulation?.();
        }}><Gamepad2 size={16} />Emulator Setup</button
      >{/if}
    {#if onuninstall}<button
        role="menuitem"
        disabled={!!uninstallUnavailable}
        title={uninstallUnavailable || "Uninstall the selected version"}
        onclick={() => {
          dismiss();
          onuninstall?.();
        }}><Trash2 size={16} />Uninstall…</button
      >{/if}
  {/if}
</div>

<style>
  .game-menu {
    position: fixed;
    inset: auto;
    margin: 0;
    width: 236px;
    padding: 6px;
    max-height: calc(100vh - 24px);
    overflow: auto;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 10px;
    box-shadow: 0 12px 32px var(--shadow);
  }
  .game-menu button {
    display: flex;
    justify-content: flex-start;
    gap: 10px;
    width: 100%;
    padding: 12px 10px;
    border-radius: 6px;
    font-size: 13px;
    text-align: left;
  }
  .game-menu button:hover:not(:disabled),
  .game-menu button:focus-visible {
    background: var(--surface-hover);
    outline-offset: -2px;
  }
  .game-menu button :global(svg) {
    color: var(--muted);
  }
</style>
