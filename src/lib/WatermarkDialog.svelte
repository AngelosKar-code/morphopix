<script lang="ts">
  import { untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Modal from "./Modal.svelte";
  import type { BatchSettings } from "./settings";
  import type { WatermarkPosition } from "./api";

  type Props = {
    settings: BatchSettings;
    onsave: (draft: WatermarkDraft) => void;
    oncancel: () => void;
  };

  export type WatermarkDraft = {
    watermarkPath: string;
    watermarkPosition: WatermarkPosition;
    watermarkSizePercent: number;
    watermarkOpacityPercent: number;
    watermarkMarginPercent: number;
  };

  let { settings, onsave, oncancel }: Props = $props();

  /**
   * Edits a copy, so dismissing the dialog with Cancel, the close button or a click outside
   * leaves the pipeline exactly as it was.
   */
  // Read once on purpose: this dialog is mounted fresh each time it opens, and these values
  // are the baseline that Cancel restores and that "unsaved changes" compares against.
  const original: WatermarkDraft = untrack(() => ({
    watermarkPath: settings.watermarkPath,
    watermarkPosition: settings.watermarkPosition,
    watermarkSizePercent: settings.watermarkSizePercent,
    watermarkOpacityPercent: settings.watermarkOpacityPercent,
    watermarkMarginPercent: settings.watermarkMarginPercent,
  }));

  let draft = $state<WatermarkDraft>({ ...original });

  const changed = $derived(
    draft.watermarkPath !== original.watermarkPath ||
      draft.watermarkPosition !== original.watermarkPosition ||
      draft.watermarkSizePercent !== original.watermarkSizePercent ||
      draft.watermarkOpacityPercent !== original.watermarkOpacityPercent ||
      draft.watermarkMarginPercent !== original.watermarkMarginPercent,
  );

  const positions: WatermarkPosition[][] = [
    ["top-left", "top-center", "top-right"],
    ["center-left", "center", "center-right"],
    ["bottom-left", "bottom-center", "bottom-right"],
  ];

  const fileName = $derived(draft.watermarkPath.split(/[\\/]/).pop() ?? "");

  async function chooseLogo() {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Logo", extensions: ["png", "webp", "jpg", "jpeg"] }],
    });
    if (typeof selected === "string") draft.watermarkPath = selected;
  }

  /** Mirrors the anchor maths in the Rust side, purely for the layout diagram. */
  function anchorStyle(position: WatermarkPosition): string {
    const [vertical, horizontal] = position.split("-");
    const margin = `${draft.watermarkMarginPercent}%`;
    const width = `${draft.watermarkSizePercent}%`;

    const x =
      horizontal === "left"
        ? `left: ${margin};`
        : horizontal === "right"
          ? `right: ${margin};`
          : `left: 50%; transform: translateX(-50%);`;
    const y =
      vertical === "top"
        ? `top: ${margin};`
        : vertical === "bottom"
          ? `bottom: ${margin};`
          : `top: 50%;`;

    const centreBoth = position === "center" ? "transform: translate(-50%, -50%);" : "";
    const centreVertical =
      vertical === "center" && horizontal !== "center" ? "transform: translateY(-50%);" : "";

    return `${x} ${y} width: ${width}; ${centreBoth || centreVertical}`;
  }
</script>

<Modal
  title="Watermark"
  subtitle="Size and margin are shares of each image's width, so the logo stays proportional across mixed resolutions."
  width="max-w-3xl"
  onclose={oncancel}
>
  <div class="grid gap-6 md:grid-cols-[1fr_minmax(0,320px)]">
    <!-- Controls -->
    <div class="space-y-4">
      <div>
        <span class="label">Logo file</span>
        <button onclick={chooseLogo} class="field field-path">
          {fileName || "Choose a PNG with transparency…"}
        </button>
      </div>

      <div>
        <span class="label">Position</span>
        <div class="inline-grid grid-cols-3 gap-1.5">
          {#each positions as row (row[0])}
            {#each row as position (position)}
              <button
                onclick={() => (draft.watermarkPosition = position)}
                aria-label={position}
                title={position.replace("-", " ")}
                class="h-8 w-8 rounded-md border transition {draft.watermarkPosition ===
                position
                  ? 'border-accent-500 bg-accent-500/25'
                  : 'border-line bg-surface-control hover:border-line-strong'}"
              ></button>
            {/each}
          {/each}
        </div>
      </div>

      <div>
        <div class="mb-1.5 flex justify-between text-xs">
          <span class="text-neutral-400">Size</span>
          <span class="tabular-nums text-neutral-200">{draft.watermarkSizePercent}%</span>
        </div>
        <input
          type="range"
          min="1"
          max="100"
          bind:value={draft.watermarkSizePercent}
          class="w-full accent-accent-500"
        />
      </div>

      <div>
        <div class="mb-1.5 flex justify-between text-xs">
          <span class="text-neutral-400">Opacity</span>
          <span class="tabular-nums text-neutral-200">{draft.watermarkOpacityPercent}%</span>
        </div>
        <input
          type="range"
          min="0"
          max="100"
          bind:value={draft.watermarkOpacityPercent}
          class="w-full accent-accent-500"
        />
      </div>

      <div>
        <div class="mb-1.5 flex justify-between text-xs">
          <span class="text-neutral-400">Margin</span>
          <span class="tabular-nums text-neutral-200">{draft.watermarkMarginPercent}%</span>
        </div>
        <input
          type="range"
          min="0"
          max="25"
          bind:value={draft.watermarkMarginPercent}
          class="w-full accent-accent-500"
        />
      </div>
    </div>

    <!-- Layout diagram: shows placement, not pixels -->
    <div>
      <span class="label">Placement</span>
      <div
        class="relative aspect-4/3 w-full overflow-hidden rounded-lg border border-line bg-[repeating-conic-gradient(#1e1e1e_0%_25%,#171717_0%_50%)] bg-[length:16px_16px]"
      >
        {#if draft.watermarkPath}
          <img
            src={`data:image/svg+xml;utf8,${encodeURIComponent(
              `<svg xmlns="http://www.w3.org/2000/svg" width="120" height="40"><rect width="120" height="40" rx="6" fill="%23a78bfa"/></svg>`,
            )}`}
            alt=""
            class="absolute"
            style="{anchorStyle(draft.watermarkPosition)} opacity: {draft.watermarkOpacityPercent / 100};"
          />
        {:else}
          <div
            class="absolute rounded-md bg-accent-400/80"
            style="{anchorStyle(draft.watermarkPosition)} aspect-ratio: 3 / 1; opacity: {draft.watermarkOpacityPercent / 100};"
          ></div>
        {/if}
      </div>
      <p class="mt-2 text-[11px] leading-snug text-neutral-500">
        Indicative only — the real logo is composited at full output resolution. Use Compare
        on a queued image to see the actual result.
      </p>
    </div>
  </div>

  {#snippet footer()}
    {#if draft.watermarkPath === ""}
      <span class="mr-auto text-xs text-warn-400">Choose a logo file to apply a watermark.</span>
    {:else if changed}
      <span class="mr-auto text-xs text-neutral-500">Unsaved changes</span>
    {/if}
    <button onclick={oncancel} class="btn">Cancel</button>
    <button
      onclick={() => onsave($state.snapshot(draft))}
      class="btn btn-primary h-8 px-5 text-[13px]"
    >
      Save
    </button>
  {/snippet}
</Modal>
