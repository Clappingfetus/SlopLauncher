<script lang="ts">
  import { FolderPlus, FolderOpen, X, LoaderCircle } from "@lucide/svelte";
  import { desktop, chooseLibraryFolder } from "../lib/backend";
  import { addLibraryFolder } from "../lib/folders";
  import { sourceNames, type AccountSource } from "../lib/types";
  let {
    source,
    paths = [],
    disabled = false,
    onchange,
    canBrowse = desktop,
    pick = chooseLibraryFolder,
  }: {
    source: AccountSource;
    paths?: string[];
    disabled?: boolean;
    onchange?: (source: AccountSource, paths: string[]) => Promise<void>;
    canBrowse?: boolean;
    pick?: () => Promise<string | null>;
  } = $props();
  let pending = $state(false),
    error = $state("");
  const hints: Record<AccountSource, string> = {
    steam: "Choose a Steam library folder or its steamapps folder.",
    epic: "Choose a library or game folder containing Epic installation records.",
    gog: "Choose a library or game folder containing goggame .info files.",
    itch: "Choose an itch library or game folder with installation receipts.",
    amazon: "Amazon installation records cover registered games across drives.",
  };
  async function add() {
    if (pending || disabled || !canBrowse || !onchange) return;
    pending = true;
    error = "";
    try {
      const path = await pick();
      if (path)
        await onchange(
          source,
          addLibraryFolder({ [source]: paths }, source, path),
        );
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    } finally {
      pending = false;
    }
  }
  async function remove(path: string) {
    if (pending || disabled || !canBrowse || !onchange) return;
    pending = true;
    error = "";
    try {
      await onchange(
        source,
        paths.filter((p) => p !== path),
      );
    } catch (e) {
      error = String(e).replace(/^Error: /, "");
    } finally {
      pending = false;
    }
  }
</script>

<section
  class="source-folders"
  aria-label={`${sourceNames[source]} library folders`}
>
  <div class="folder-heading">
    <span><FolderOpen size={13} />Library folders</span>
    <button
      class="text-button"
      disabled={disabled || !canBrowse || pending || !onchange}
      onclick={add}
      aria-label={`Add ${sourceNames[source]} library folder`}
    >
      {#if pending}<LoaderCircle size={13} class="spin" />{:else}<FolderPlus
          size={13}
        />{/if}Add folder</button
    >
  </div>
  {#if paths.length}<ul>
      {#each paths as path (path)}<li>
          <code title={path}>{path}</code>
          <button
            class="icon-button"
            disabled={disabled || !canBrowse || pending || !onchange}
            onclick={() => remove(path)}
            aria-label={`Remove ${sourceNames[source]} folder ${path}`}
            title="Remove folder from list"><X size={13} /></button
          >
        </li>{/each}
    </ul>{:else}<p>
      Automatic discovery is on. Add more folders as needed.
    </p>{/if}
  <p class="folder-hint">{hints[source]}</p>
  {#if error}<p class="form-error" role="alert">{error}</p>{/if}
  <p
    class="field-hint"
    title="Removing a folder only changes discovery locations; it does not delete files."
  >
    Add each library location separately. Folders locate installed games;
    connecting an account imports owned games.
  </p>
</section>
