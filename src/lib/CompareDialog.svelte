<script lang="ts">
  import { Maximize, Minimize, Minus, Plus } from "@lucide/svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import Modal from "./Modal.svelte";
  import BeforeAfter from "./BeforeAfter.svelte";
  import { previewFile, type PreviewResult, type ProcessOptions } from "./api";
  import { formatBytes, savingsPercent } from "./format";

  type Props = {
    path: string;
    fileName: string;
    options: ProcessOptions;
    onclose: () => void;
  };

  let { path, fileName, options, onclose }: Props = $props();

  let preview = $state<PreviewResult | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);
  let appFullscreen = $state(false);

  /** null means "fit the frame"; a number is an explicit zoom factor. */
  let zoom = $state<number | null>(null);
  let zoomInput = $state("");
  let frameWidth = $state(0);
  let frameHeight = $state(0);

  const MIN_ZOOM = 0.05;
  const MAX_ZOOM = 8;
  const ZOOM_STEPS = [0.05, 0.1, 0.25, 0.33, 0.5, 0.7, 1, 1.5, 2, 3, 4, 6, 8];

  /** Rendered generously so zooming past fit still has real detail to show. */
  const RENDER_SIZE = 2400;

  /**
   * Fit has to respect the frame's height as well as its width — a tall image that only
   * satisfies the width still runs off the bottom of the panel.
   */
  const fitZoom = $derived.by(() => {
    if (!preview || frameWidth <= 0 || frameHeight <= 0) return 1;
    return Math.min(
      frameWidth / preview.outputWidth,
      frameHeight / preview.outputHeight,
      1,
    );
  });
  const effectiveZoom = $derived(zoom ?? fitZoom);
  const displayWidth = $derived(
    preview ? Math.max(1, Math.round(preview.outputWidth * effectiveZoom)) : 0,
  );

  // Only runs while the dialog is open, so a batch never pays for previews it never shows.
  $effect(() => {
    let cancelled = false;
    loading = true;

    previewFile(path, options, RENDER_SIZE)
      .then((result) => {
        if (cancelled) return;
        preview = result;
        error = null;
      })
      .catch((reason) => {
        if (!cancelled) error = String(reason);
      })
      .finally(() => {
        if (!cancelled) loading = false;
      });

    return () => {
      cancelled = true;
    };
  });

  // Leaving the dialog must not leave the window stuck in full screen.
  $effect(() => {
    void getCurrentWindow()
      .isFullscreen()
      .then((value) => (appFullscreen = value));

    return () => {
      void getCurrentWindow().setFullscreen(false);
    };
  });

  const zoomPercent = $derived(Math.round(effectiveZoom * 100));

  function setZoom(value: number) {
    zoom = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value));
  }

  function stepZoom(direction: 1 | -1) {
    const current = effectiveZoom;
    const candidates = direction === 1 ? ZOOM_STEPS : [...ZOOM_STEPS].reverse();
    const next = candidates.find((step) =>
      direction === 1 ? step > current + 0.001 : step < current - 0.001,
    );
    setZoom(next ?? current);
  }

  function commitTypedZoom() {
    const parsed = Number.parseFloat(zoomInput.replace("%", "").trim());
    if (Number.isFinite(parsed) && parsed > 0) setZoom(parsed / 100);
    zoomInput = "";
  }

  /**
   * Full screen means the whole window, not just this panel: filling a half-sized window
   * with a lightbox does nothing for how much of the image you can actually see.
   */
  async function toggleAppFullscreen() {
    const next = !appFullscreen;
    await getCurrentWindow().setFullscreen(next);
    appFullscreen = next;
  }
</script>

<Modal title="Compare" subtitle={fileName} width="max-w-none h-full" {onclose}>
  {#if error}
    <p class="py-12 text-center text-sm text-danger-400">{error}</p>
  {:else if preview}
    <div class="flex h-full flex-col gap-3">
      <!-- Zoom controls -->
      <div class="flex shrink-0 flex-wrap items-center gap-2 text-xs">
        <button onclick={() => stepZoom(-1)} class="btn h-7 px-2" aria-label="Zoom out">
          <Minus size={14} />
        </button>

        <input
          type="range"
          min={Math.round(MIN_ZOOM * 100)}
          max={Math.round(MAX_ZOOM * 100)}
          value={zoomPercent}
          oninput={(event) => setZoom(Number(event.currentTarget.value) / 100)}
          aria-label="Zoom"
          class="w-40 accent-accent-500"
        />

        <button onclick={() => stepZoom(1)} class="btn h-7 px-2" aria-label="Zoom in">
          <Plus size={14} />
        </button>

        <input
          type="text"
          value={zoomInput === "" ? `${zoomPercent}%` : zoomInput}
          oninput={(event) => (zoomInput = event.currentTarget.value)}
          onblur={commitTypedZoom}
          onkeydown={(event) => event.key === "Enter" && commitTypedZoom()}
          aria-label="Zoom percentage"
          class="field w-20 text-center tabular-nums"
        />

        <button
          onclick={() => (zoom = null)}
          class="btn h-7 {zoom === null ? 'border-accent-500 text-accent-300' : ''}"
          title="Scale the image to fit the panel"
        >
          Fit
        </button>
        <button
          onclick={() => setZoom(1)}
          class="btn h-7 {zoom === 1 ? 'border-accent-500 text-accent-300' : ''}"
          title="Show the exported pixels one to one"
        >
          Actual size
        </button>

        <span class="ml-1 tabular-nums text-neutral-500">
          {preview.outputWidth}×{preview.outputHeight}
        </span>

        <button
          onclick={toggleAppFullscreen}
          class="btn ml-auto h-7"
          title={appFullscreen ? "Restore the window" : "Put the whole window full screen"}
        >
          {#if appFullscreen}
            <Minimize size={14} /> Exit full screen
          {:else}
            <Maximize size={14} /> Full screen
          {/if}
        </button>
      </div>

      <!-- Viewport: scrolls once the image is larger than the frame -->
      <div
        bind:clientWidth={frameWidth}
        bind:clientHeight={frameHeight}
        class="min-h-0 flex-1 overflow-auto rounded-xl border border-line bg-surface-app p-3"
      >
        <div style="width: {displayWidth}px; margin: 0 auto;">
          <BeforeAfter
            beforeUri={preview.beforeUri}
            afterUri={preview.afterUri}
            stale={loading}
          />
        </div>
      </div>

      <div class="grid shrink-0 grid-cols-2 gap-3 sm:grid-cols-4">
        <div class="card px-3.5 py-2.5">
          <span class="block text-[11px] text-neutral-500">Dimensions</span>
          <span class="text-sm tabular-nums text-neutral-100">
            {preview.originalWidth}×{preview.originalHeight} → {preview.outputWidth}×{preview.outputHeight}
          </span>
        </div>
        <div class="card px-3.5 py-2.5">
          <span class="block text-[11px] text-neutral-500">File size</span>
          <span class="text-sm tabular-nums text-neutral-100">
            {formatBytes(preview.originalBytes)} → {formatBytes(preview.outputBytes)}
          </span>
        </div>
        <div class="card px-3.5 py-2.5">
          <span class="block text-[11px] text-neutral-500">Saving</span>
          <span class="text-sm font-semibold tabular-nums text-success-400">
            {savingsPercent(preview.originalBytes, preview.outputBytes)}%
          </span>
        </div>
        <div class="card px-3.5 py-2.5">
          <span class="block text-[11px] text-neutral-500">Applies to</span>
          <span class="text-sm text-neutral-100">every queued image</span>
        </div>
      </div>
    </div>
  {:else}
    <p class="py-16 text-center text-sm text-neutral-500">Rendering both versions…</p>
  {/if}
</Modal>
