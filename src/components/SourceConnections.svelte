<script lang="ts">
  import {
    ArrowUpRight,
    RefreshCw,
    LoaderCircle,
    X,
    ExternalLink,
    ShieldCheck,
  } from "@lucide/svelte";
  import { sourceNames, type Game, type AccountSource } from "../lib/types";
  import {
    accountSources,
    type AccountState,
    type AccountManager,
  } from "../lib/accounts";
  import { desktop, openResource as openExternal } from "../lib/backend";
  import { onDestroy } from "svelte";
  import ConnectionHelp from "./ConnectionHelp.svelte";
  import SourceFolders from "./SourceFolders.svelte";
  import type { LibraryFolders } from "../lib/types";
  let {
    games,
    only,
    accounts,
    manager,
    disabled = false,
    steamId = "",
    canConnect = desktop,
    folders = {},
    onFoldersChange,
    onSteamHistory,
    steamHistoryBusy = false,
    steamHistoryMessage = "",
  }: {
    games: Game[];
    onSteamHistory?: () => void;
    steamHistoryBusy?: boolean;
    steamHistoryMessage?: string;
    only?: AccountSource;
    accounts: AccountState[];
    manager: AccountManager;
    disabled?: boolean;
    steamId?: string;
    canConnect?: boolean;
    folders?: LibraryFolders;
    onFoldersChange?: (source: AccountSource, paths: string[]) => Promise<void>;
  } = $props();
  let selected = $state<AccountSource | null>(null),
    linkError = $state(""),
    code = $state(""),
    key = $state(""),
    steam = $state(""),
    dialog: HTMLDialogElement;
  let connection = $derived(accounts.find((a) => a.source === selected));
  let working = $derived(
    connection &&
      ["opening", "connecting", "syncing", "disconnecting"].includes(
        connection.phase,
      ),
  );
  $effect(() => {
    if (selected && dialog && !dialog.open) dialog.showModal();
    if (!selected && dialog?.open) dialog.close();
  });
  onDestroy(() => {
    if (selected && connection?.phase === "signing-in")
      void manager.cancel(selected);
  });
  async function connect(source: AccountSource) {
    linkError = "";
    selected = source;
    code = "";
    key = "";
    steam = steamId;
    if (source !== "steam" && source !== "itch") await manager.begin(source);
  }
  function close() {
    if ((selected === "steam" || selected === "itch") && working) return;
    if (selected && connection?.phase !== "syncing")
      void manager.cancel(selected);
    selected = null;
    code = "";
    key = "";
  }
  async function openResource(url: string) {
    try {
      linkError = "";
      await openExternal(url);
    } catch (e) {
      linkError = `Could not open link: ${e}`;
    }
  }
  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!selected || working) return;
    const source = selected;
    const input = source === "steam" || source === "itch" ? key : code;
    key = "";
    code = "";
    const success =
      source === "steam"
        ? await manager.steam(steam.trim(), input.trim())
        : source === "itch"
          ? await manager.itch(input.trim())
          : await manager.finish(source, input.trim());
    if (success && selected === source) {
      selected = null;
      key = "";
      code = "";
    }
  }
  function time(value?: number) {
    return value
      ? new Intl.DateTimeFormat(undefined, {
          month: "short",
          day: "numeric",
          hour: "numeric",
          minute: "2-digit",
        }).format(value)
      : "Not synced yet";
  }
</script>

{#if !only}
  <div class="account-toolbar">
    <span
      >Connected libraries follow your background sync interval in Settings.</span
    >
    <button
      class="secondary-button"
      disabled={disabled || !canConnect || !accounts.some((a) => a.connected)}
      onclick={() => manager.syncAll()}
      ><RefreshCw size={14} />Sync connected sources</button
    >
  </div>
{/if}
<div class="source-panels">
  {#each accountSources.filter((source) => !only || source === only) as source}
    {@const account = accounts.find((a) => a.source === source)!}
    {@const busy = [
      "opening",
      "connecting",
      "syncing",
      "disconnecting",
    ].includes(account.phase)}
    <section
      class="source-panel"
      aria-label={`${sourceNames[source]} connection`}
    >
      <div class="source-panel-top">
        <span class="store-icon large" data-source={source}
          >{source === "gog"
            ? "gog"
            : source === "amazon"
              ? "a"
              : source === "steam"
                ? "S"
                : source === "itch"
                  ? "i"
                  : "E"}</span
        >
        <div>
          <h2>{sourceNames[source]}</h2>
          <span
            >{games.filter((g) => g.copies.some((c) => c.source === source))
              .length} games in library</span
          >
        </div>
      </div>
      <p>
        {source === "steam"
          ? "Connect your SteamID64 and Web API key to import owned games, including those you haven’t installed."
          : source === "itch"
            ? "Connect your itch.io API key to import purchases, claims, and games from owned bundles."
            : `Sign in to ${sourceNames[source]} in your browser to import your complete owned collection.`}
      </p>
      {#if source === "steam"}<p class="field-hint">
          Steam play history refreshes every 15 minutes when background sync is
          enabled. You can refresh it manually at any time. SlopLauncher keeps
          its own launch history.
        </p>
        {#if steamHistoryMessage}<p class="field-hint" role="status">
            {steamHistoryMessage}
          </p>{/if}{/if}
      <ConnectionHelp service={source} expanded={!!only} />
      {#if account.connected}
        <div class="account-summary">
          <strong>{account.displayName ?? sourceNames[source]}</strong><span
            >{account.ownedCount ?? "—"} owned · {time(account.lastSync)}</span
          >
        </div>
      {/if}
      {#if account.error}<p class="form-error account-error" role="status">
          {account.error}
        </p>{/if}
      <div class="source-panel-footer">
        <span class="connection-state"
          ><span
            class="status-dot"
            class:pending={!account.connected || account.reconnectRequired}
          ></span>{account.phase === "syncing"
            ? "Importing owned games…"
            : account.phase === "disconnecting"
              ? "Disconnecting…"
              : account.reconnectRequired
                ? "Reconnect needed"
                : account.connected
                  ? "Connected"
                  : "Not connected"}</span
        >
        <div class="account-actions">
          {#if account.connected}
            {#if source === "steam" && onSteamHistory}<button
                class="text-button"
                disabled={disabled || busy || steamHistoryBusy}
                onclick={onSteamHistory}
                >{steamHistoryBusy
                  ? "Refreshing history…"
                  : "Refresh play history"}</button
              >{/if}
            <button
              class="text-button"
              disabled={disabled ||
                !canConnect ||
                account.phase === "disconnecting"}
              onclick={() => manager.disconnect(source)}>Disconnect</button
            >
          {/if}
          {#if account.connected && !account.reconnectRequired}
            <button
              class="secondary-button"
              disabled={disabled || !canConnect || busy}
              onclick={() => manager.sync(source)}
              >{#if busy}<LoaderCircle
                  size={14}
                  class="spin"
                />{:else}<RefreshCw size={14} />{/if}Sync library</button
            >
          {:else}
            <button
              class="secondary-button"
              disabled={disabled || !canConnect || busy}
              onclick={() => connect(source)}
              >{account.reconnectRequired
                ? "Reconnect"
                : "Connect"}<ArrowUpRight size={14} /></button
            >
          {/if}
        </div>
      </div>
      <SourceFolders
        {source}
        paths={folders[source] ?? []}
        disabled={disabled || !onFoldersChange}
        onchange={onFoldersChange}
      />
    </section>
  {/each}
</div>

<dialog
  class="account-dialog"
  bind:this={dialog!}
  oncancel={(event) => {
    if ((selected === "steam" || selected === "itch") && working)
      event.preventDefault();
    else close();
  }}
  onclose={() => {
    selected = null;
    key = "";
    code = "";
  }}
  aria-labelledby="account-dialog-title"
>
  {#if selected}
    <div class="dialog-header">
      <h2 id="account-dialog-title">Connect {sourceNames[selected]}</h2>
      <button
        class="icon-button"
        aria-label="Close connection dialog"
        disabled={(selected === "steam" || selected === "itch") && working}
        onclick={close}><X size={19} /></button
      >
    </div>
    <form onsubmit={submit}>
      {#if linkError}<p class="form-error" role="alert">{linkError}</p>{/if}
      {#if selected === "steam"}
        <p class="dialog-description">
          Import your owned library and keep it updated automatically. Your game
          details must be visible to Steam’s API.
        </p>
        <label
          >SteamID64<input
            bind:value={steam}
            required
            pattern={"[0-9]{17}"}
            placeholder="Your 17-digit Steam ID"
            autocomplete="off"
            aria-describedby="steam-id-help"
          /></label
        >
        <a
          class="text-button"
          href="https://store.steampowered.com/account/"
          onclick={(event) => {
            event.preventDefault();
            void openResource("https://store.steampowered.com/account/");
          }}>Find my SteamID64<ExternalLink size={12} /></a
        >
        <p class="field-hint" id="steam-id-help">
          Sign in to Steam, then copy the 17-digit Steam ID beneath your account
          name on Account Details.
        </p>
        <label
          >Steam Web API key<input
            bind:value={key}
            required
            type="password"
            autocomplete="new-password"
            placeholder="Saved securely for future syncs"
          /></label
        >
        <button
          type="button"
          class="text-button"
          onclick={() => openResource("https://steamcommunity.com/dev/apikey")}
          >Get your Steam API key<ExternalLink size={12} /></button
        >
      {:else if selected === "itch"}
        <p class="dialog-description">
          Import purchased and claimed games, including unclaimed games in owned
          bundles. New games are picked up automatically on later syncs.
        </p>
        <label
          >itch.io API key<input
            bind:value={key}
            required
            type="password"
            autocomplete="new-password"
            placeholder="Saved securely for future syncs"
          /></label
        >
        <button
          type="button"
          class="text-button"
          onclick={() => openResource("https://itch.io/user/settings/api-keys")}
          >Get your itch.io API key<ExternalLink size={12} /></button
        >
        <p class="field-hint">
          Use a personal API key from your itch.io settings. Installation and
          launching use the official itch app.
        </p>
      {:else}
        <p class="dialog-description">
          Finish signing in on {sourceNames[selected]}’s website, then return
          here. Your password stays in your browser.
        </p>
        <ol class="login-steps">
          <li>Complete the sign-in opened in your browser.</li>
          <li>
            {selected === "epic"
              ? "Copy the authorizationCode value (or the complete JSON response)."
              : selected === "gog"
                ? "Copy the final address containing code= from your browser’s address bar."
                : "Copy the final amazon.com address containing openid.oa2.authorization_code from your browser’s address bar."}
          </li>
          <li>Paste it below to connect this launcher.</li>
        </ol>
        <button
          type="button"
          class="secondary-button"
          disabled={working}
          onclick={() => manager.reopen(selected!)}
          ><ExternalLink size={14} />{connection?.ticket
            ? "Open sign-in again"
            : "Start sign-in again"}</button
        >
        <label class="login-code-label"
          >{selected === "epic"
            ? "Authorization code or JSON"
            : "Sign-in completion URL"}<textarea
            bind:value={code}
            required
            autocomplete="off"
            autocapitalize="off"
            spellcheck="false"
            rows="3"
            placeholder={selected === "epic"
              ? "Paste the authorization code"
              : "Paste the complete redirect URL"}></textarea></label
        >
        <p class="field-hint">
          The temporary sign-in expires after 10 minutes. Paste it only here,
          never into chat.
        </p>
      {/if}
      <p class="account-security">
        <ShieldCheck size={14} />Credentials are protected by Windows for your
        user.
      </p>
      {#if connection?.error}<p class="form-error" role="alert">
          {connection.error}
        </p>{/if}
      <div class="dialog-footer">
        <button
          type="button"
          class="secondary-button"
          disabled={(selected === "steam" || selected === "itch") && working}
          onclick={close}>Cancel</button
        ><button
          class="primary-button"
          disabled={disabled ||
            !canConnect ||
            working ||
            (selected !== "steam" &&
              selected !== "itch" &&
              !connection?.ticket)}
          >{#if working}<LoaderCircle
              size={14}
              class="spin"
            />{/if}{connection?.phase === "syncing"
            ? "Importing games…"
            : "Connect & import"}</button
        >
      </div>
    </form>
  {/if}
</dialog>
