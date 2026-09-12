<script lang="ts">
  import { ChevronDown, FolderOutput } from "@lucide/svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import type { Destination } from "./destination";

  type Props = {
    destination: Destination;
    /** Where results would actually be written, for the explanatory line. */
    resolved: string;
  };

  let { destination, resolved }: Props = $props();

  let menuOpen = $state(false);

  const label = $derived(
    destination.mode === "subfolder"
      ? destination.subfolderName.trim() || "optimized"
      : (destination.customFolder.split(/[\\/]/).pop() ?? "Choose…"),
  );

  async function chooseCustomFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      destination.customFolder = selected;
      destination.mode = "custom";
    }
  }
</script>

<svelte:window onkeydown={(event) => event.key === "Escape" && (menuOpen = false)} />

<div class="relative">
  <button
    onclick={() => (menuOpen = !menuOpen)}
    class="btn min-w-0 max-w-64 shrink gap-2"
    aria-haspopup="dialog"
    aria-expanded={menuOpen}
    title={resolved || "Choose where results are written"}
  >
    <FolderOutput size={14} class="shrink-0 text-neutral-500" />
    <span class="shrink-0 text-neutral-500">Output</span>
    <span class="truncate">{label}</span>
    <ChevronDown size={14} class="shrink-0 text-neutral-500" />
  </button>

  {#if menuOpen}
    <div class="fixed inset-0 z-40" role="presentation" onclick={() => (menuOpen = false)}></div>

    <div
      class="absolute left-1/2 z-50 mt-1.5 w-80 -translate-x-1/2 space-y-3 rounded-xl border border-line bg-surface-panel p-3 shadow-2xl"
    >
      <label class="flex cursor-pointer items-start gap-2">
        <input
          type="radio"
          value="subfolder"
          bind:group={destination.mode}
          class="mt-1 accent-accent-500"
        />
        <span class="min-w-0 flex-1">
          <span class="block text-[13px] text-neutral-200">Subfolder of the source</span>
          <span class="mb-1.5 block text-[11px] text-neutral-500">
            Keeps originals untouched next to the results.
          </span>
          <input
            type="text"
            bind:value={destination.subfolderName}
            onfocus={() => (destination.mode = "subfolder")}
            class="field"
          />
        </span>
      </label>

      <div class="border-t border-line"></div>

      <label class="flex cursor-pointer items-start gap-2">
        <input
          type="radio"
          value="custom"
          bind:group={destination.mode}
          class="mt-1 accent-accent-500"
        />
        <span class="min-w-0 flex-1">
          <span class="block text-[13px] text-neutral-200">Another folder</span>
          <span class="mb-1.5 block text-[11px] text-neutral-500">
            Needed when the images come from several places.
          </span>
          <button
            onclick={chooseCustomFolder}
            class="field field-path {destination.customFolder ? '' : 'text-neutral-500'}"
            title={destination.customFolder}
          >
            {destination.customFolder || "Choose folder…"}
          </button>
        </span>
      </label>

      <p class="truncate border-t border-line pt-2.5 text-[11px] text-neutral-500" title={resolved}>
        {resolved || "No destination yet"}
      </p>
    </div>
  {/if}
</div>
