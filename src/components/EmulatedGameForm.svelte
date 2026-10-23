<script lang="ts">
  import { untrack } from "svelte";
  import { FolderOpen, File, Settings2 } from "@lucide/svelte";
  import {
    consoles,
    type EmulatorProfile,
    type EmulatedGameInput,
    type ConsoleId,
  } from "../lib/emulation";
  import * as api from "../lib/backend";
  let {
    profiles,
    initial,
    onsubmit,
    oncancel,
    onsetup,
  }: {
    profiles: EmulatorProfile[];
    initial?: EmulatedGameInput;
    onsubmit: (input: EmulatedGameInput) => Promise<void>;
    oncancel: () => void;
    onsetup: () => void;
  } = $props();
  const start = untrack(() => ({
    title: initial?.title ?? "",
    profileId: initial?.profileId ?? profiles[0]?.id ?? "",
    gamePath: initial?.gamePath ?? "",
    platform:
      profiles.find((p) => p.id === initial?.profileId)?.platform ??
      profiles[0]?.platform ??
      "ps1",
  }));
  let title = $state(start.title),
    profileId = $state(start.profileId),
    gamePath = $state(start.gamePath),
    platform = $state<ConsoleId>(start.platform),
    busy = $state(false),
    error = $state("");
  let available = $derived(profiles.filter((p) => p.platform === platform));
  async function choose(folder = false) {
    if (busy) return;
    const requestedPlatform = platform;
    try {
      const result = await (folder
        ? api.chooseLibraryFolder()
        : api.chooseEmulatedFile(platform));
      if (result && requestedPlatform === platform) {
        gamePath = result;
        if (!title && !folder && !/eboot\./i.test(result))
          title =
            result
              .split(/[\\/]/)
              .at(-1)
              ?.replace(/\.[^.]+$/, "") ?? "";
      }
    } catch (e) {
      error = String(e);
    }
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    error = "";
    busy = true;
    try {
      await onsubmit({ title: title.trim(), profileId, gamePath });
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<form onsubmit={submit}>
  <p class="dialog-description">
    Add a game image, multi-disc playlist, or installed console game.
  </p>
  <label
    >Game title<input
      bind:value={title}
      required
      maxlength="300"
      placeholder="Game name"
    /></label
  >
  <label
    >Console<select
      aria-label="Game console"
      bind:value={platform}
      onchange={() =>
        (profileId = profiles.find((p) => p.platform === platform)?.id ?? "")}
      >{#each consoles as console}<option value={console.id}
          >{console.name}</option
        >{/each}</select
    ></label
  >
  {#if available.length}<label
      >Emulator profile<select
        aria-label="Game emulator profile"
        bind:value={profileId}
        >{#each available as profile}<option value={profile.id}
            >{profile.name}</option
          >{/each}</select
      ></label
    >{:else}<div class="setup-hint">
      <p>No emulator profile for this console yet.</p>
      <button type="button" class="text-button" onclick={onsetup}
        ><Settings2 size={14} />Set up emulators</button
      >
    </div>{/if}
  <label
    >Game file or folder
    <div class="input-with-button">
      <input
        aria-label="Game file or folder"
        bind:value={gamePath}
        required
        placeholder={platform === "ps3"
          ? "D:\\Games\\PS3\\Game\\PS3_GAME\\USRDIR\\EBOOT.BIN"
          : platform === "ps4"
            ? "D:\\Games\\PS4\\Game\\eboot.bin"
            : platform === "wiiu"
              ? "D:\\Games\\WiiU\\Game\\code\\game.rpx"
              : "D:\\Games\\Console\\game image"}
      /><button
        type="button"
        class="secondary-button"
        aria-label="Choose emulated game file"
        onclick={() => choose()}><File size={15} /></button
      ><button
        type="button"
        class="secondary-button"
        aria-label="Choose emulated game folder"
        onclick={() => choose(true)}><FolderOpen size={15} /></button
      >
    </div></label
  >
  <p class="field-hint">
    {platform === "ps3"
      ? "Choose the installed game's EBOOT.BIN or its game folder. Install packages in RPCS3 first."
      : platform === "ps4"
        ? "Choose eboot.bin or its game folder. Use the shadPS4 emulator executable, not its updater."
        : platform === "wiiu"
          ? "Choose WUA, WUD/WUX, or RPX. A game folder with one code/*.rpx also works."
          : platform === "switch"
            ? "Choose an NSP, XCI, or homebrew NRO. Complete your emulator's setup first."
            : platform === "ps1"
              ? "Use a CUE/CHD image or an M3U playlist for multi-disc games."
              : "The selected emulator must already be configured to run this game."}
  </p>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
  <div class="dialog-footer">
    <button
      type="button"
      class="secondary-button"
      disabled={busy}
      onclick={oncancel}>Cancel</button
    ><button
      class="primary-button"
      disabled={busy || !profileId || !api.desktop}
      >{initial ? "Save launch settings" : "Add to library"}</button
    >
  </div>
</form>

<style>
  select {
    width: 100%;
    padding: 11px 12px;
    margin-top: 8px;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .setup-hint {
    border: 1px solid var(--line);
    padding: 12px;
    margin-top: 16px;
    border-radius: 6px;
  }
  .setup-hint p {
    font-size: 12px;
    color: var(--muted);
    margin-bottom: 8px;
  }
</style>
