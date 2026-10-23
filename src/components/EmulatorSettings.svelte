<script lang="ts">
  import ConnectionHelp from "./ConnectionHelp.svelte";
  import { Plus, Pencil, Trash2, FolderOpen, X } from "@lucide/svelte";
  import {
    consoles,
    consoleName,
    presetsFor,
    emulatorPresets,
    type ConsoleId,
    type EmulatorKind,
    type EmulatorProfile,
  } from "../lib/emulation";
  import type { Game } from "../lib/types";
  import * as api from "../lib/backend";
  let {
    profiles,
    games,
    onchange,
    disabled = false,
  }: {
    profiles: EmulatorProfile[];
    games: Game[];
    onchange: (profiles: EmulatorProfile[]) => Promise<void>;
    disabled?: boolean;
  } = $props();
  let editing = $state(false),
    id = $state(""),
    name = $state(""),
    platform = $state<ConsoleId>("ps1"),
    kind = $state<EmulatorKind>("duckstation"),
    executable = $state(""),
    core = $state(""),
    args = $state(""),
    error = $state(""),
    busy = $state(false);
  const used = (profileId: string) =>
    games.filter((g) =>
      g.copies.some((c) => c.emulation?.profileId === profileId),
    ).length;
  function edit(profile?: EmulatorProfile) {
    if (busy || disabled) return;
    id = profile?.id ?? crypto.randomUUID();
    name = profile?.name ?? "";
    platform = profile?.platform ?? "ps1";
    kind = profile?.kind ?? "duckstation";
    executable = profile?.executable ?? "";
    core = profile?.corePath ?? "";
    args = (profile?.arguments ?? emulatorPresets[0].args).join("\n");
    error = "";
    editing = true;
  }
  function preset(next: EmulatorKind) {
    kind = next;
    args = emulatorPresets.find((p) => p.id === kind)!.args.join("\n");
  }
  async function choose(corePicker = false) {
    if (busy || disabled || !editing) return;
    const target = [id, platform, kind].join(":");
    try {
      const result = await (corePicker
        ? api.chooseEmulatorCore()
        : api.chooseExecutable());
      if (result && editing && target === [id, platform, kind].join(":")) {
        if (corePicker) core = result;
        else executable = result;
      }
    } catch (e) {
      error = String(e);
    }
  }
  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (busy || disabled) return;
    error = "";
    busy = true;
    try {
      const profile: EmulatorProfile = {
        id,
        name: name.trim(),
        platform,
        kind,
        executable,
        corePath: core || undefined,
        arguments: args
          .split(/\r?\n/)
          .map((a) => a.trim())
          .filter(Boolean),
      };
      await api.validateEmulatorProfile(profile);
      await onchange(
        profiles.some((p) => p.id === id)
          ? profiles.map((p) => (p.id === id ? profile : p))
          : [...profiles, profile],
      );
      editing = false;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
  async function remove(profileId: string) {
    if (used(profileId) || busy || disabled) return;
    error = "";
    busy = true;
    try {
      await onchange(profiles.filter((p) => p.id !== profileId));
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="emulator-settings" aria-labelledby="emulators-heading">
  <div class="emulator-heading">
    <div>
      <h3 id="emulators-heading">Emulators</h3>
      <p>One profile for each emulator and console.</p>
    </div>
    <button
      class="secondary-button"
      disabled={disabled || busy || editing || !api.desktop}
      onclick={() => edit()}><Plus size={14} />Add profile</button
    >
  </div>
  <ConnectionHelp service="emulators" />
  {#if profiles.length}<ul>
      {#each profiles as profile (profile.id)}<li>
          <div>
            <strong>{profile.name}</strong><span
              >{consoleName(profile.platform)} · {used(profile.id)} games</span
            >
          </div>
          <button
            class="icon-button"
            aria-label={`Edit ${profile.name}`}
            disabled={disabled || busy || editing}
            onclick={() => edit(profile)}><Pencil size={14} /></button
          ><button
            class="icon-button"
            aria-label={`Remove ${profile.name}`}
            disabled={disabled || busy || editing || !!used(profile.id)}
            title={used(profile.id)
              ? "Choose a different profile for its games before removing this profile."
              : "Remove emulator profile"}
            onclick={() => remove(profile.id)}><Trash2 size={14} /></button
          >
        </li>{/each}
    </ul>{:else if !editing}<p class="field-hint">
      Add a profile, then choose Emulated game under Add game.
    </p>{/if}
  {#if editing}<form class="emulator-editor" onsubmit={save}>
      <fieldset disabled={busy || disabled}>
        <label
          >Profile name<input
            bind:value={name}
            required
            maxlength="200"
            placeholder="e.g. PlayStation · DuckStation"
          /></label
        >
        <label
          >Console<select
            aria-label="Profile console"
            bind:value={platform}
            disabled={!!used(id)}
            onchange={() => preset(presetsFor(platform)[0].id)}
            >{#each consoles as console}<option value={console.id}
                >{console.name}</option
              >{/each}</select
          ></label
        >
        <label
          >Emulator<select
            aria-label="Emulator preset"
            value={kind}
            onchange={(e) => preset(e.currentTarget.value as EmulatorKind)}
            >{#each presetsFor(platform) as emulator}<option value={emulator.id}
                >{emulator.name}</option
              >{/each}</select
          ></label
        >
        <label
          >Emulator executable
          <div class="input-with-button">
            <input
              aria-label="Emulator executable"
              bind:value={executable}
              required
              placeholder="C:\Emulators\emulator.exe"
            /><button
              type="button"
              class="secondary-button"
              aria-label="Choose emulator executable"
              onclick={() => choose()}><FolderOpen size={15} /></button
            >
          </div></label
        >
        {#if kind === "retroarch"}<label
            >RetroArch core DLL
            <div class="input-with-button">
              <input
                aria-label="RetroArch core DLL"
                bind:value={core}
                required
                placeholder="C:\RetroArch\cores\core_libretro.dll"
              /><button
                type="button"
                class="secondary-button"
                aria-label="Choose RetroArch core"
                onclick={() => choose(true)}><FolderOpen size={15} /></button
              >
            </div></label
          >
          <p class="field-hint">
            Select an installed core for this console. RetroArch compatibility
            depends on that core.
          </p>{/if}
        <details>
          <summary>Launch arguments</summary><label
            >Arguments, one per line<textarea
              aria-label="Emulator launch arguments"
              bind:value={args}
              rows="4"
              required></textarea></label
          >
          <p class="field-hint">
            Use {"{game}"} for the game path and {"{core}"} for a core. Each line
            is one argument; paths do not need quotes.
          </p>
        </details>
        <div class="editor-actions">
          <button
            class="secondary-button"
            type="button"
            disabled={busy}
            onclick={() => (editing = false)}><X size={14} />Cancel</button
          ><button class="primary-button" disabled={busy}>Save profile</button>
        </div>
      </fieldset>
    </form>{/if}
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
</section>

<style>
  .emulator-settings {
    padding: 22px 0;
    border-bottom: 1px solid var(--line);
  }
  .emulator-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }
  h3 {
    font-size: 13px;
    font-weight: 500;
    margin: 0;
  }
  p {
    font-size: 11px;
    color: var(--faint);
    line-height: 1.7;
    margin-top: 5px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 14px 0 0;
  }
  li {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 12px 0;
    border-top: 1px solid var(--line);
  }
  li > div {
    flex: 1;
    min-width: 0;
  }
  strong {
    display: block;
    font-size: 12px;
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  li span {
    display: block;
    font-size: 11px;
    color: var(--faint);
    margin-top: 5px;
  }
  .emulator-editor {
    margin-top: 18px;
    padding: 14px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--surface);
  }
  select {
    width: 100%;
    margin-top: 8px;
    padding: 10px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: 6px;
  }
  .editor-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }
  summary {
    font-size: 12px;
    cursor: pointer;
    margin-top: 16px;
  }
  textarea {
    font-family: ui-monospace, monospace;
    font-size: 12px;
  }
</style>
