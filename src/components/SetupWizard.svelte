<script lang="ts">
  import SourceConnections from "./SourceConnections.svelte";
  import ProviderSettings from "./ProviderSettings.svelte";
  import EmulatorSettings from "./EmulatorSettings.svelte";
  import {
    setupServices,
    type SetupId,
    type ProviderSetupId,
  } from "../lib/setup";
  import {
    accountSources,
    type AccountManager,
    type AccountState,
  } from "../lib/accounts";
  import type {
    Game,
    Settings,
    ProviderStatus,
    AccountSource,
  } from "../lib/types";
  import type { EmulatorProfile } from "../lib/emulation";
  let {
    games,
    settings,
    providers,
    accounts,
    manager,
    disabled,
    onproviders,
    onfolders,
    onemulators,
    onclose,
  }: {
    games: Game[];
    settings: Settings;
    providers: ProviderStatus;
    accounts: AccountState[];
    manager: AccountManager;
    disabled: boolean;
    onproviders: (status: ProviderStatus) => void;
    onfolders: (source: AccountSource, paths: string[]) => Promise<void>;
    onemulators: (profiles: EmulatorProfile[]) => Promise<void>;
    onclose: () => void;
  } = $props();
  let chosen = $state<SetupId[]>([]);
  let index = $state(-1);
  let steps = $derived(setupServices.filter((s) => chosen.includes(s.id)));
  let current = $derived(steps[index]);
  function connected(id: SetupId): boolean {
    if (id === "emulators") return !!settings.emulators?.length;
    if (id === "steamgriddb") return providers.steamGridDbConfigured;
    if (id === "igdb") return providers.igdbConfigured;
    if (id === "screenscraper") return !!providers.screenScraperConfigured;
    if (id === "retroachievements")
      return providers.retroAchievementsConfigured ?? false;
    return accounts.some(
      (a) => a.source === id && a.connected && !a.reconnectRequired,
    );
  }
</script>

<main class="settings-page setup-page" aria-label="Setup wizard">
  <div class="wizard-heading">
    <div>
      <div class="page-eyebrow">OPTIONAL SETUP</div>
      <h1>Set up your library</h1>
    </div>
    <button class="secondary-button" onclick={onclose}>Exit setup</button>
  </div>
  <p class="intro">
    Choose what you use. Skip any step; saved connections stay connected. You
    can return from Settings or Sources anytime.
  </p>
  {#if index === -1}
    <h2>Choose your connections</h2>
    <div class="choices">
      {#each setupServices as service}<label class="choice"
          ><input type="checkbox" value={service.id} bind:group={chosen} /><span
            ><strong>{service.name}</strong><small>{service.purpose}</small
            ><small class="state"
              >{connected(service.id)
                ? "Already configured"
                : "Optional"}</small
            ></span
          ></label
        >{/each}
    </div>
    <div class="wizard-footer">
      <button class="text-button" onclick={onclose}>Skip setup</button><button
        class="primary-button"
        disabled={!steps.length || disabled}
        onclick={() => (index = 0)}>Start setup ({steps.length})</button
      >
    </div>
  {:else if current}
    <div class="step-heading">
      <span>Step {index + 1} of {steps.length}</span><span
        >{connected(current.id) ? "Configured" : "Not configured"}</span
      >
    </div>
    <progress value={index} max={steps.length} aria-label="Setup progress"
    ></progress>
    <h2>{current.name}</h2>
    <p class="intro">{current.purpose}</p>
    {#key current.id}
      {#if accountSources.includes(current.id as AccountSource)}
        <SourceConnections
          {games}
          {accounts}
          {manager}
          {disabled}
          only={current.id as AccountSource}
          steamId={settings.steamId}
          folders={settings.libraryFolders ?? {}}
          onFoldersChange={onfolders}
        />
      {:else if current.id === "emulators"}
        <EmulatorSettings
          {games}
          {disabled}
          profiles={settings.emulators ?? []}
          onchange={onemulators}
        />
      {:else}
        <ProviderSettings
          only={current.id as ProviderSetupId}
          status={providers}
          onchange={onproviders}
        />
      {/if}
    {/key}
    <p class="save-note">
      Save or connect above before continuing. Next and Skip do not save
      unfinished forms.
    </p>
    <div class="wizard-footer">
      <button class="secondary-button" onclick={() => index--}>Back</button
      ><button class="text-button" onclick={() => index++}
        >Skip this step</button
      ><button class="primary-button" onclick={() => index++}>Next</button>
    </div>
  {:else}
    <h2>Setup reviewed</h2>
    <p class="intro">
      Your saved connections are ready. Unfinished connections can be configured
      anytime.
    </p>
    <ul class="review">
      {#each steps as step}<li>
          <span>{step.name}</span><strong
            >{connected(step.id) ? "Configured" : "Not configured"}</strong
          >
        </li>{/each}
    </ul>
    <p class="intro">
      Use Sources → Scan installed games to find local installations. Add game
      lets you add manual or emulated games. No account is needed for manual
      games. HowLongToBeat and PCGamingWiki links appear in game details without
      separate logins.
    </p>
    <div class="wizard-footer">
      <button class="secondary-button" onclick={() => (index = -1)}
        >Change selection</button
      ><button class="primary-button" onclick={onclose}>Finish setup</button>
    </div>
  {/if}
</main>

<style>
  .setup-page {
    max-width: 1120px;
  }
  .wizard-heading,
  .wizard-footer,
  .step-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .wizard-heading h1 {
    margin: 8px 0;
  }
  .intro,
  .save-note {
    color: var(--faint);
    font-size: 13px;
    line-height: 1.7;
    max-width: 780px;
  }
  h2 {
    font-size: 18px;
    font-weight: 500;
    margin: 25px 0 12px;
  }
  .choices {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }
  .choice {
    display: flex;
    gap: 14px;
    align-items: start;
    padding: 16px;
    border: 1px solid var(--line);
    border-radius: 9px;
    cursor: pointer;
    background: var(--surface);
  }
  .choice:has(input:checked) {
    border-color: var(--accent);
  }
  .choice input {
    width: 16px;
    height: 16px;
    margin-top: 3px;
  }
  strong {
    font-size: 13px;
    font-weight: 500;
  }
  small {
    display: block;
    font-size: 12px;
    color: var(--faint);
    line-height: 1.6;
    margin-top: 4px;
  }
  .state {
    font-size: 11px;
  }
  .wizard-footer {
    justify-content: flex-end;
    border-top: 1px solid var(--line);
    padding-top: 18px;
    margin-top: 24px;
  }
  .step-heading {
    font-size: 12px;
    margin-top: 24px;
    color: var(--faint);
  }
  progress {
    width: 100%;
    height: 4px;
    accent-color: var(--accent);
    margin: 10px 0;
  }
  .review {
    padding: 0;
    list-style: none;
  }
  .review li {
    display: flex;
    justify-content: space-between;
    padding: 14px 0;
    border-bottom: 1px solid var(--line);
    font-size: 13px;
  }
  @media (max-width: 800px) {
    .choices {
      grid-template-columns: 1fr;
    }
  }
</style>
