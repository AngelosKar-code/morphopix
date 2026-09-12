<script lang="ts">
  import { Bookmark, BookmarkPlus, ChevronDown, X } from "@lucide/svelte";
  import type { PresetStore } from "./presets.svelte";
  import { builtinTemplates, type BatchSettings } from "./settings";

  type Props = {
    presets: PresetStore;
    settings: BatchSettings;
    active: string;
    /** The steps have been changed since this template was applied. */
    edited?: boolean;
    onapply: (name: string) => void;
    onapplybuiltin: (name: string, settings: BatchSettings) => void;
    onactivechange: (name: string) => void;
  };

  let {
    presets,
    settings,
    active,
    edited = false,
    onapply,
    onapplybuiltin,
    onactivechange,
  }: Props = $props();

  let open = $state(false);
  let naming = $state(false);
  let draftName = $state("");

  function close() {
    open = false;
    cancelNaming();
  }

  function cancelNaming() {
    naming = false;
    draftName = "";
  }

  function startNaming() {
    naming = true;
    draftName = active;
  }

  async function confirmSave() {
    const name = draftName.trim();
    if (name === "") return;
    await presets.save(name, $state.snapshot(settings));
    onactivechange(name);
    cancelNaming();
    open = false;
  }

  async function remove(name: string) {
    await presets.remove(name);
    if (active === name) onactivechange("");
  }
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && close()} />

<div class="relative">
  <!-- The label stays even once a template is chosen: on its own, a bare name gives no clue
       what kind of thing it is. -->
  <button
    onclick={() => (open = !open)}
    class="btn min-w-0 max-w-64 shrink gap-2"
    aria-haspopup="menu"
    aria-expanded={open}
    title="Saved combinations of processing steps"
  >
    <Bookmark size={14} class="shrink-0 text-neutral-500" />
    <span class="shrink-0 text-neutral-500">Template</span>
    <span class="truncate {active ? 'text-neutral-100' : 'text-neutral-500'}">
      {active || "None"}{active && edited ? " (edited)" : ""}
    </span>
    <ChevronDown size={14} class="shrink-0 text-neutral-500" />
  </button>

  {#if open}
    <!-- Click-away layer; keeps the menu dismissable without a global listener. -->
    <div class="fixed inset-0 z-40" role="presentation" onclick={close}></div>

    <div
      role="menu"
      class="absolute left-1/2 z-50 mt-1.5 w-72 -translate-x-1/2 overflow-hidden rounded-xl border border-line bg-surface-panel shadow-2xl"
    >
      <div class="border-b border-line py-1">
        <span class="block px-3 pb-1 pt-1.5 text-[10px] uppercase tracking-wide text-neutral-600">
          Built in
        </span>
        {#each builtinTemplates as template (template.name)}
          <button
            role="menuitem"
            onclick={() => {
              onapplybuiltin(template.name, template.settings);
              close();
            }}
            class="block w-full px-3 py-1.5 text-left transition hover:bg-surface-control"
          >
            <span
              class="block text-[13px] {active === template.name
                ? 'text-accent-400'
                : 'text-neutral-200'}"
            >
              {template.name}
            </span>
            <span class="block text-[11px] leading-snug text-neutral-500">
              {template.hint}
            </span>
          </button>
        {/each}
      </div>

      {#if presets.names.length > 0}
        <div class="max-h-64 overflow-y-auto py-1">
          <span
            class="block px-3 pb-1 pt-1.5 text-[10px] uppercase tracking-wide text-neutral-600"
          >
            Yours
          </span>
          {#each presets.names as name (name)}
            <div class="group flex items-center pr-1.5 transition hover:bg-surface-control">
              <button
                role="menuitem"
                onclick={() => {
                  onapply(name);
                  close();
                }}
                class="flex-1 truncate px-3 py-2 text-left text-[13px] {active === name
                  ? 'text-accent-400'
                  : 'text-neutral-300'}"
              >
                {name}
              </button>
              <button
                onclick={() => remove(name)}
                aria-label="Delete template {name}"
                title="Delete template"
                class="rounded p-1 text-neutral-600 opacity-0 transition group-hover:opacity-100 hover:text-danger-400"
              >
                <X size={13} />
              </button>
            </div>
          {/each}
        </div>
      {/if}

      <div class="border-t border-line p-2">
        {#if naming}
          <input
            type="text"
            placeholder="Template name"
            bind:value={draftName}
            onkeydown={(event) => {
              if (event.key === "Enter") confirmSave();
              if (event.key === "Escape") cancelNaming();
            }}
            class="field mb-2"
          />
          <div class="flex gap-2">
            <button
              onclick={confirmSave}
              disabled={draftName.trim() === ""}
              class="btn flex-1"
            >
              Save
            </button>
            <button onclick={cancelNaming} class="btn flex-1">Cancel</button>
          </div>
        {:else}
          <button onclick={startNaming} class="btn w-full">
            <BookmarkPlus size={14} /> Save current steps
          </button>
        {/if}
      </div>
    </div>
  {/if}
</div>
