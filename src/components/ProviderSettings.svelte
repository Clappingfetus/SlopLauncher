<script lang="ts">
  import ConnectionHelp from "./ConnectionHelp.svelte";
  import type { ProviderSetupId } from "../lib/setup";
  import { ExternalLink, LoaderCircle } from "@lucide/svelte";
  import type { ProviderStatus, ProviderConfigInput } from "../lib/types";
  import {
    desktop,
    saveProviderConfig,
    testProvider,
    openResource as openExternal,
  } from "../lib/backend";
  let {
    status,
    onchange,
    only,
  }: {
    status: ProviderStatus;
    onchange: (s: ProviderStatus) => void;
    only?: ProviderSetupId;
  } = $props();
  let sgdb = $state(""),
    raUsername = $state(""),
    raKey = $state(""),
    clientId = $state(""),
    secret = $state(""),
    ssDeveloperId = $state(""),
    ssDeveloperPassword = $state(""),
    ssUsername = $state(""),
    ssPassword = $state(""),
    busy = $state(""),
    message = $state(""),
    error = $state(false);
  async function openResource(url: string) {
    try {
      await openExternal(url);
    } catch (e) {
      message = String(e);
      error = true;
    }
  }
  async function save(
    input: ProviderConfigInput = {
      steamGridDbKey: sgdb,
      retroAchievements: { username: raUsername, apiKey: raKey },
      igdbClientId: clientId,
      igdbClientSecret: secret,
      screenScraper: {
        developerId: ssDeveloperId,
        developerPassword: ssDeveloperPassword,
        username: ssUsername,
        password: ssPassword,
      },
    },
  ) {
    if (busy) return;
    busy = "save";
    message = "";
    error = false;
    try {
      const updated = await saveProviderConfig(input);
      onchange(updated);
      sgdb = "";
      raUsername = raKey = "";
      secret = "";
      clientId = "";
      ssDeveloperId = ssDeveloperPassword = ssUsername = ssPassword = "";
      message =
        "Saved securely. Saved connections will be used when game data is requested.";
    } catch (e) {
      message = String(e);
      error = true;
    } finally {
      busy = "";
    }
  }
  async function test(
    provider: "igdb" | "steamgriddb" | "screenscraper" | "retroachievements",
  ) {
    if (busy) return;
    busy = provider;
    message = "";
    try {
      message = await testProvider(provider);
      error = false;
    } catch (e) {
      message = String(e);
      error = true;
    } finally {
      busy = "";
    }
  }
</script>

<section class="provider-settings" aria-label="Metadata providers">
  {#if !only}
    <h3>Metadata, artwork & achievements</h3>
    <p class="muted">
      Store artwork stays the default. SteamGridDB fills missing PC covers;
      emulator artwork is only changed when you choose it in Change Artwork.
    </p>
    <p>Enriches every game, including future imports and manual additions.</p>
  {/if}
  {#if status.warning}<p class="form-error">{status.warning}</p>{/if}
  {#if !only || only === "steamgriddb"}
    <div class="provider-setting">
      <div class="provider-setting-title">
        <strong>SteamGridDB</strong><span
          >{status.steamGridDbConfigured ? "Key saved" : "Key needed"}</span
        >
      </div>
      <ConnectionHelp service="steamgriddb" expanded={!!only} />
      <label
        >API key<input
          type="password"
          autocomplete="new-password"
          bind:value={sgdb}
          placeholder={status.steamGridDbConfigured
            ? "Saved · leave blank to keep"
            : "SteamGridDB API key"}
        /></label
      >
      <div class="provider-setting-actions">
        <button
          class="text-button"
          onclick={() =>
            openResource("https://www.steamgriddb.com/profile/preferences/api")}
          >Get an API key <ExternalLink size={12} /></button
        ><button
          class="text-button"
          disabled={!!busy || !desktop || !status.steamGridDbConfigured}
          onclick={() => test("steamgriddb")}>Test connection</button
        >{#if status.steamGridDbConfigured}<button
            class="text-button"
            disabled={!!busy}
            onclick={() => save({ clearSteamGridDb: true })}>Disconnect</button
          >{/if}
      </div>
    </div>
  {/if}
  {#if !only || only === "igdb"}
    <div class="provider-setting">
      <div class="provider-setting-title">
        <strong>IGDB</strong><span
          >{status.igdbConfigured
            ? "Credentials saved"
            : "Credentials needed"}</span
        >
      </div>
      <ConnectionHelp service="igdb" expanded={!!only} />
      <label
        >Twitch client ID<input
          autocomplete="off"
          bind:value={clientId}
          placeholder={status.igdbClientId ??
            "Twitch developer application client ID"}
        /></label
      ><label
        >Client secret<input
          type="password"
          autocomplete="new-password"
          bind:value={secret}
          placeholder={status.igdbConfigured
            ? "Saved · leave blank to keep"
            : "Twitch application client secret"}
        /></label
      >
      <div class="provider-setting-actions">
        <button
          class="text-button"
          onclick={() => openResource("https://dev.twitch.tv/console/apps")}
          >Twitch applications <ExternalLink size={12} /></button
        ><button
          class="text-button"
          disabled={!!busy || !desktop || !status.igdbConfigured}
          onclick={() => test("igdb")}>Test connection</button
        >{#if status.igdbConfigured}<button
            class="text-button"
            disabled={!!busy}
            onclick={() => save({ clearIgdb: true })}>Disconnect</button
          >{/if}
      </div>
    </div>
  {/if}
  {#if !only || only === "screenscraper"}
    <div class="provider-setting">
      <div class="provider-setting-title">
        <strong>ScreenScraper</strong><span
          >{status.screenScraperConfigured
            ? "Credentials saved"
            : "Credentials needed"}</span
        >
      </div>
      <p class="field-hint">
        Console metadata and box art for existing and newly added emulated
        games. API access requires ScreenScraper developer credentials; a
        personal login alone is insufficient.
      </p>
      <ConnectionHelp service="screenscraper" expanded={!!only} />
      <label
        >Developer ID<input
          aria-label="ScreenScraper developer ID"
          type="password"
          autocomplete="new-password"
          bind:value={ssDeveloperId}
          placeholder={status.screenScraperConfigured
            ? "Saved · leave blank to keep"
            : "ScreenScraper developer ID"}
        /></label
      >
      <label
        >Developer password<input
          aria-label="ScreenScraper developer password"
          type="password"
          autocomplete="new-password"
          bind:value={ssDeveloperPassword}
          placeholder={status.screenScraperConfigured
            ? "Saved · leave blank to keep"
            : "ScreenScraper developer password"}
        /></label
      >
      <label
        >Username (optional)<input
          aria-label="ScreenScraper username"
          autocomplete="off"
          bind:value={ssUsername}
          placeholder="Leave blank to keep a saved login"
        /></label
      >
      <label
        >Password (optional)<input
          aria-label="ScreenScraper password"
          type="password"
          autocomplete="new-password"
          bind:value={ssPassword}
          placeholder="Leave blank to keep a saved login"
        /></label
      >
      <div class="provider-setting-actions">
        <button
          class="text-button"
          onclick={() =>
            openResource("https://www.screenscraper.fr/webapi2.php")}
          >API access & documentation <ExternalLink size={12} /></button
        >
        <button
          class="text-button"
          disabled={!!busy || !desktop || !status.screenScraperConfigured}
          onclick={() => test("screenscraper")}
          >Test ScreenScraper connection</button
        >
        {#if status.screenScraperConfigured}<button
            class="text-button"
            disabled={!!busy}
            onclick={() => save({ clearScreenScraper: true })}
            >Disconnect ScreenScraper</button
          >{/if}
      </div>
    </div>
  {/if}
  {#if !only || only === "retroachievements"}
    <div class="provider-setting" aria-label="RetroAchievements connection">
      <div class="provider-setting-title">
        <strong>RetroAchievements</strong><span
          >{status.retroAchievementsConfigured
            ? `Connected · ${status.retroAchievementsUsername ?? "Account saved"}`
            : "Not connected"}</span
        >
      </div>
      <p class="field-hint">
        Regular and Hardcore achievements for linked emulated games. Sign in to
        RetroAchievements inside your compatible emulator to earn unlocks.
      </p>
      <ConnectionHelp service="retroachievements" expanded={!!only} />
      <label
        >Username<input
          aria-label="RetroAchievements username"
          autocomplete="off"
          bind:value={raUsername}
          placeholder={status.retroAchievementsUsername ??
            "RetroAchievements username"}
        /></label
      >
      <label
        >Web API key<input
          aria-label="RetroAchievements Web API key"
          type="password"
          autocomplete="new-password"
          bind:value={raKey}
          placeholder={status.retroAchievementsConfigured
            ? "Saved · leave blank to keep"
            : "Web API key"}
        /></label
      >
      <div class="provider-setting-actions">
        <button
          class="text-button"
          onclick={() =>
            openResource("https://retroachievements.org/controlpanel.php")}
          >Get RetroAchievements API key <ExternalLink size={12} /></button
        >
        <button
          class="text-button"
          disabled={!!busy || !desktop || !status.retroAchievementsConfigured}
          onclick={() => test("retroachievements")}
          >Test RetroAchievements connection</button
        >
        {#if status.retroAchievementsConfigured}<button
            class="text-button"
            disabled={!!busy}
            onclick={() => save({ clearRetroAchievements: true })}
            >Disconnect RetroAchievements</button
          >{/if}
      </div>
    </div>
  {/if}
  <button
    class="primary-button"
    disabled={!!busy || !desktop}
    onclick={() => save()}
    >{#if busy === "save"}<LoaderCircle size={14} class="spin" />{/if}Save
    provider settings</button
  >
  <p class="field-hint">
    Keys are protected by Windows for this user. Game details include provider
    attribution.
  </p>
  {#if message}<p
      class:form-error={error}
      class="provider-message"
      role="status"
    >
      {message}
    </p>{/if}
</section>
