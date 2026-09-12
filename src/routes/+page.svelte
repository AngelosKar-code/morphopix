<script lang="ts">
  import { untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { getVersion } from "@tauri-apps/api/app";
  import {
    ArrowRight,
    FileImage,
    FileOutput,
    FolderInput,
    FolderOpen,
    Images,
    Lock,
    Maximize2,
    Play,
    RotateCcw,
    Square,
    Stamp,
    Tags,
    Trash2,
    TriangleAlert,
    Type,
    UploadCloud,
    X,
  } from "@lucide/svelte";
  import { openPath } from "@tauri-apps/plugin-opener";
  import {
    cancelBatch,
    collectDroppedPaths,
    cpuCount,
    listImagesInFolder,
    onBatchProgress,
    processBatch,
    type BatchSummary,
  } from "$lib/api";
  import { formatBytes, savingsPercent } from "$lib/format";
  import {
    defaultSettings,
    threadsFor,
    toProcessOptions,
    type BatchSettings,
  } from "$lib/settings";
  import {
    commonParentFolder,
    defaultDestination,
    resolveOutputFolder,
    type Destination,
  } from "$lib/destination";
  import { previewOutputName } from "$lib/naming";
  import { PresetStore } from "$lib/presets.svelte";
  import FileList, { type Row } from "$lib/FileList.svelte";
  import Stage from "$lib/Stage.svelte";
  import TemplateMenu from "$lib/TemplateMenu.svelte";
  import DestinationMenu from "$lib/DestinationMenu.svelte";
  import WatermarkDialog from "$lib/WatermarkDialog.svelte";
  import CompareDialog from "$lib/CompareDialog.svelte";
  import ConfirmDialog from "$lib/ConfirmDialog.svelte";

  let inputFolder = $state("");
  let recursive = $state(false);
  let entries = $state<Row[]>([]);
  /**
   * A folder queue can be rescanned (the subfolders toggle re-reads it); a queue the user
   * assembled by dropping or picking individual files must never be silently replaced by a
   * folder scan.
   */
  let queueSource = $state<"folder" | "files">("folder");
  let scanning = $state(false);
  let runState = $state<"idle" | "running" | "done">("idle");
  let completed = $state(0);
  let summary = $state<BatchSummary | null>(null);
  let errorMessage = $state<string | null>(null);
  let selectedPath = $state<string | null>(null);
  let dragging = $state(false);

  let destination = $state<Destination>({ ...defaultDestination });
  let settings = $state<BatchSettings>({ ...defaultSettings });
  let cores = $state(4);
  let appVersion = $state("");

  const presets = new PresetStore();
  let activePreset = $state("");
  /** Serialized steps as they were when the template was applied, to spot divergence. */
  let appliedTemplateState = $state("");

  let showWatermarkDialog = $state(false);
  let showCompareDialog = $state(false);
  let confirming = $state<"clear" | "reset" | null>(null);

  const outputFolder = $derived(resolveOutputFolder(destination, inputFolder));
  const progressPercent = $derived(
    entries.length === 0 ? 0 : Math.round((completed / entries.length) * 100),
  );
  const canStart = $derived(
    entries.length > 0 && outputFolder !== "" && runState === "idle" && !scanning,
  );
  const totalInputBytes = $derived(entries.reduce((sum, e) => sum + e.sizeBytes, 0));
  /**
   * PNG has no lossy setting at all, and lossless WebP ignores it. With conversion off the
   * target is whatever each source already is, which may well be lossy, so it still applies.
   */
  const usesQuality = $derived.by(() => {
    if (!settings.convertEnabled) return true;
    if (settings.format === "png") return false;
    if (settings.format === "webp") return !settings.webpLossless;
    return true;
  });
  const watermarkFileName = $derived(settings.watermarkPath.split(/[\\/]/).pop() ?? "");
  const selectedEntry = $derived(entries.find((entry) => entry.path === selectedPath) ?? null);
  const namePreview = $derived(
    previewOutputName(entries[0]?.fileName ?? "Product Photo.jpg", settings),
  );

  /** Reads as a sentence so the pipeline is legible without opening every card. */
  const activeSteps = $derived(
    [
      settings.convertEnabled && `to ${settings.format.toUpperCase()}`,
      settings.resizeEnabled && `max ${settings.maxWidth}×${settings.maxHeight}`,
      settings.renameEnabled && "renamed",
      settings.watermarkEnabled && settings.watermarkPath !== "" && "watermarked",
      settings.preserveMetadata && "metadata kept",
    ].filter(Boolean) as string[],
  );

  $effect(() => {
    void cpuCount().then((count) => (cores = count));
    void getVersion().then((version) => (appVersion = `v${version}`));
    void presets.init();
  });

  // A stale error banner is worse than none: it keeps describing a problem the user has
  // already moved past.
  $effect(() => {
    if (errorMessage === null) return;
    const timer = setTimeout(() => (errorMessage = null), 9000);
    return () => clearTimeout(timer);
  });

  // Files and folders dropped onto the window.
  $effect(() => {
    const unlistenPromise = getCurrentWebview().onDragDropEvent(async (event) => {
      if (event.payload.type === "over") {
        dragging = true;
        return;
      }
      if (event.payload.type === "leave") {
        dragging = false;
        return;
      }

      dragging = false;
      const paths = event.payload.paths;
      if (paths.length === 0) return;

      scanning = true;
      try {
        const found = await collectDroppedPaths(
          paths,
          recursive,
          untrack(() => outputFolder) || null,
        );
        // Drops add to the queue rather than replacing it, so several folders can be
        // gathered before a single run.
        appendToQueue(found, paths.some((path) => !/\.[^\\/.]+$/.test(path)));
        errorMessage = null;
      } catch (error) {
        errorMessage = String(error);
      } finally {
        scanning = false;
      }
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  });

  /** A finished run no longer describes what the current settings would produce. */
  const settingsKey = $derived(JSON.stringify(settings));
  $effect(() => {
    settingsKey;
    if (untrack(() => runState) === "done") reset();
  });

  $effect(() => {
    const unlistenPromise = onBatchProgress((progress) => {
      const entry = entries.find((item) => item.path === progress.path);
      if (entry) {
        entry.status = progress.status;
        entry.outputBytes = progress.outputBytes;
        entry.error = progress.error;
      }
      completed = progress.completed;
    });

    return () => {
      unlistenPromise.then((unlisten) => unlisten());
    };
  });

  function toRow(item: { path: string; fileName: string; sizeBytes: number }): Row {
    return { ...item, status: "idle" as const, outputBytes: 0, error: null };
  }

  /**
   * Merges newly added images into the queue and re-derives the source folder from
   * everything now queued, so the destination subfolder lands where the user expects.
   */
  function appendToQueue(
    found: { path: string; fileName: string; sizeBytes: number }[],
    fromFolders: boolean,
  ) {
    const known = new Set(entries.map((entry) => entry.path));
    const added = found.filter((item) => !known.has(item.path)).map(toRow);
    if (added.length === 0 && entries.length > 0) return;

    entries = [...entries, ...added].sort((a, b) =>
      a.fileName.toLowerCase().localeCompare(b.fileName.toLowerCase()),
    );
    queueSource = fromFolders ? "folder" : "files";
    inputFolder = commonParentFolder(entries.map((entry) => entry.path));
    selectedPath ??= entries[0]?.path ?? null;
    reset();
  }

  async function chooseInputFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return;
    inputFolder = selected;
    queueSource = "folder";
    await rescan();
  }

  async function addFiles() {
    const selected = await open({
      multiple: true,
      filters: [
        { name: "Images", extensions: ["jpg", "jpeg", "png", "webp", "bmp", "tiff", "gif"] },
      ],
    });
    const paths = Array.isArray(selected) ? selected : selected ? [selected] : [];
    if (paths.length === 0) return;

    scanning = true;
    try {
      const found = await collectDroppedPaths(paths, false, outputFolder || null);
      appendToQueue(found, false);
      errorMessage = null;
    } catch (error) {
      errorMessage = String(error);
    } finally {
      scanning = false;
    }
  }

  /** Re-reads the source folder. Only valid for a folder-derived queue. */
  async function rescan() {
    if (!inputFolder || queueSource !== "folder") return;
    scanning = true;
    reset();

    try {
      const found = await listImagesInFolder(inputFolder, recursive, outputFolder || null);
      entries = found.map(toRow);
      selectedPath = entries[0]?.path ?? null;
      errorMessage = null;
    } catch (error) {
      errorMessage = String(error);
    } finally {
      scanning = false;
    }
  }

  function reset() {
    runState = "idle";
    completed = 0;
    summary = null;
    for (const entry of entries) {
      entry.status = "idle";
      entry.outputBytes = 0;
      entry.error = null;
    }
  }

  function clearQueue() {
    entries = [];
    selectedPath = null;
    inputFolder = "";
    queueSource = "folder";
    reset();
  }

  /** Back to a blank slate: default steps and an empty queue. */
  function resetEverything() {
    settings = { ...defaultSettings, performance: settings.performance };
    activePreset = "";
    destination = { ...defaultDestination };
    clearQueue();
  }

  async function openOutputFolder() {
    if (!outputFolder) return;
    try {
      await openPath(outputFolder);
    } catch (error) {
      // Most often the folder does not exist yet because nothing has been processed.
      errorMessage = `Cannot open ${outputFolder} — it may not exist yet. ${error}`;
    }
  }

  function removeEntry(path: string) {
    entries = entries.filter((entry) => entry.path !== path);
    if (selectedPath === path) selectedPath = entries[0]?.path ?? null;
  }

  /** Built-ins keep the machine's performance choice, which is not part of a template. */
  function applyBuiltinTemplate(name: string, template: BatchSettings) {
    settings = { ...template, performance: settings.performance };
    activePreset = name;
    appliedTemplateState = JSON.stringify(settings);
  }

  async function applyPreset(name: string) {
    activePreset = name;
    if (name === "") {
      appliedTemplateState = "";
      return;
    }
    const stored = await presets.get(name);
    if (stored) {
      settings = stored;
      appliedTemplateState = JSON.stringify(stored);
    }
  }

  async function start() {
    if (!canStart) return;

    reset();
    runState = "running";
    errorMessage = null;

    try {
      summary = await processBatch(
        entries.map((entry) => entry.path),
        outputFolder,
        toProcessOptions(settings),
        threadsFor(settings.performance, cores),
      );
      runState = "done";
    } catch (error) {
      runState = "idle";
      errorMessage = String(error);
    }
  }
</script>

<div class="flex h-screen flex-col overflow-hidden bg-surface-app text-neutral-100">
  <!-- Top bar. The identity block matches the steps column width so the path controls
       begin exactly where the queue begins. -->
  <header
    class="relative z-30 flex h-14 shrink-0 items-stretch border-b border-line bg-surface-panel"
  >
    <div class="flex w-[22rem] shrink-0 items-center gap-2.5 border-r border-line px-4">
      <img src="/logo.png" alt="" class="h-7 w-7 shrink-0 rounded-lg" />
      <span class="text-[15px] font-semibold tracking-tight">MorphoPix</span>
      <!-- Read from the bundle rather than hardcoded, so it cannot drift from the build. -->
      <span class="mt-0.5 self-center text-[11px] tabular-nums text-neutral-600">
        {appVersion}
      </span>
    </div>

    <div class="flex min-w-0 flex-1 items-center gap-2 px-5">
      <button
        onclick={chooseInputFolder}
        class="btn min-w-0 max-w-80 shrink gap-2"
        title={inputFolder || "Choose the folder to read images from"}
      >
        <FolderInput size={14} class="shrink-0 text-neutral-500" />
        <span class="shrink-0 text-neutral-500">Input</span>
        <span class="truncate {inputFolder ? '' : 'text-neutral-500'}">
          {inputFolder ? (inputFolder.split(/[\/]/).pop() || inputFolder) : "Choose…"}
        </span>
      </button>

      {#if queueSource === "folder"}
        <label
          class="flex shrink-0 items-center gap-1.5 text-[11px] text-neutral-400"
          title="Also read images inside subfolders"
        >
          <input
            type="checkbox"
            bind:checked={recursive}
            onchange={rescan}
            class="accent-accent-500"
          />
          Subfolders
        </label>
      {/if}

      <ArrowRight size={14} class="shrink-0 text-neutral-700" />

      <DestinationMenu {destination} resolved={outputFolder} />

      <div class="ml-auto flex shrink-0 items-center gap-2">
        <TemplateMenu
          {presets}
          {settings}
          active={activePreset}
          edited={activePreset !== "" && settingsKey !== appliedTemplateState}
          onapply={applyPreset}
          onapplybuiltin={applyBuiltinTemplate}
          onactivechange={(name) => {
            activePreset = name;
            appliedTemplateState = settingsKey;
          }}
        />

        <select
          bind:value={settings.performance}
          class="field w-[10.5rem]"
          aria-label="Performance"
        >
          <option value="maximum">Maximum · {cores} cores</option>
          <option value="balanced">Balanced · {Math.max(1, Math.ceil(cores / 2))} cores</option>
          <option value="low">Low memory · 2 cores</option>
        </select>

        <button
          onclick={() => (confirming = "reset")}
          class="btn btn-danger px-2"
          title="Reset every step to its default and empty the queue"
          aria-label="Reset everything"
        >
          <RotateCcw size={15} />
        </button>
      </div>
    </div>
  </header>

  <div class="flex min-h-0 flex-1">
    <!-- Steps: what happens to each image -->
    <section
      class="flex w-[22rem] shrink-0 flex-col gap-2.5 overflow-y-auto border-r border-line p-4"
    >
      <div class="px-1 pb-1">
        <h1 class="text-sm font-semibold">Processing steps</h1>
        <p class="mt-0.5 text-[11px] leading-snug text-neutral-500">
          {activeSteps.length > 0 ? activeSteps.join(" · ") : "Nothing enabled yet"}
        </p>
      </div>

      <!-- Format leads: it is the decision that defines the output -->
      <Stage
        title="Output format"
        icon={FileOutput}
        bind:enabled={settings.convertEnabled}
        summary="Each image keeps its own format, recompressed at the quality below."
      >
        <select bind:value={settings.format} class="field">
          <option value="webp">WebP — smallest for the web</option>
          <option value="jpeg">JPEG — widest support</option>
          <option value="png">PNG — lossless</option>
        </select>

        {#if settings.format === "webp"}
          <label class="flex items-center gap-2 text-xs text-neutral-400">
            <input
              type="checkbox"
              bind:checked={settings.webpLossless}
              class="accent-accent-500"
            />
            Lossless WebP
          </label>
        {/if}

        {#if settings.format === "png"}
          <label class="flex items-center gap-2 text-xs text-neutral-400">
            <input
              type="checkbox"
              bind:checked={settings.pngMaxCompression}
              class="accent-accent-500"
            />
            Maximum compression — slower
          </label>
        {/if}

        <label class="flex items-start gap-2 text-xs text-neutral-400">
          <input
            type="checkbox"
            bind:checked={settings.passthroughSameFormat}
            class="mt-0.5 accent-accent-500"
          />
          <span>
            Copy files already in this format
            <span class="mt-0.5 block text-[11px] text-neutral-600">
              Avoids recompressing them a second time. Ignored when a resize or watermark
              has to change the pixels.
            </span>
          </span>
        </label>

        <!-- Quality lives here because it is a property of the export. Hidden entirely for
             formats that have no lossy setting, rather than shown greyed out and puzzling. -->
        {#if usesQuality}
          <div class="border-t border-line pt-3">
            <div class="mb-1.5 flex justify-between text-xs text-neutral-400">
              <span>Quality</span>
              <span class="tabular-nums text-neutral-200">{settings.quality}</span>
            </div>
            <input
              type="range"
              min="1"
              max="100"
              bind:value={settings.quality}
              class="w-full accent-accent-500"
            />
          </div>
        {:else}
          <p class="border-t border-line pt-3 text-[11px] text-neutral-500">
            {settings.format === "png"
              ? "PNG is lossless, so there is no quality setting."
              : "Lossless WebP ignores quality."}
          </p>
        {/if}
      </Stage>

      <Stage
        title="Resize"
        icon={Maximize2}
        bind:enabled={settings.resizeEnabled}
        summary="Images keep their original dimensions."
      >
        <div class="flex items-end gap-2">
          <label class="block flex-1">
            <span class="label">Max width</span>
            <input type="number" min="1" bind:value={settings.maxWidth} class="field" />
          </label>
          <span class="pb-2 text-neutral-600" title="Aspect ratio is always preserved">
            <Lock size={13} />
          </span>
          <label class="block flex-1">
            <span class="label">Max height</span>
            <input type="number" min="1" bind:value={settings.maxHeight} class="field" />
          </label>
        </div>

        <label class="block">
          <span class="label">Resampling filter</span>
          <select bind:value={settings.filter} class="field">
            <option value="lanczos3">Lanczos3 — best quality</option>
            <option value="catmullrom">Catmull-Rom</option>
            <option value="triangle">Triangle — faster</option>
            <option value="nearest">Nearest — fastest</option>
          </select>
        </label>

        <label class="flex items-center gap-2 text-xs text-neutral-400">
          <input type="checkbox" bind:checked={settings.allowUpscale} class="accent-accent-500" />
          Allow upscaling smaller images
        </label>
      </Stage>

      <Stage
        title="Rename files"
        icon={Type}
        bind:enabled={settings.renameEnabled}
        summary="Output keeps each source filename."
      >
        <label class="block">
          <span class="label">Suffix</span>
          <input type="text" placeholder="_web" bind:value={settings.suffix} class="field" />
        </label>

        <label class="block">
          <span class="label">Spaces</span>
          <select bind:value={settings.spaceHandling} class="field">
            <option value="hyphen">Replace with - (URL friendly)</option>
            <option value="underscore">Replace with _</option>
            <option value="remove">Remove</option>
            <option value="keep">Keep as they are</option>
          </select>
        </label>

        <label class="flex items-center gap-2 text-xs text-neutral-400">
          <input
            type="checkbox"
            bind:checked={settings.lowercaseNames}
            class="accent-accent-500"
          />
          Lowercase filenames
        </label>
      </Stage>

      <Stage
        title="Watermark"
        icon={Stamp}
        bind:enabled={settings.watermarkEnabled}
        summary="No logo is applied."
      >
        {#snippet action()}
          <button onclick={() => (showWatermarkDialog = true)} class="btn h-6 px-2 text-xs">
            Configure
          </button>
        {/snippet}

        <div class="flex items-center gap-2 text-xs">
          <FileImage size={14} class="shrink-0 text-neutral-600" />
          <span
            class="min-w-0 flex-1 truncate {settings.watermarkPath
              ? 'text-neutral-200'
              : 'text-neutral-500'}"
            title={settings.watermarkPath}
          >
            {watermarkFileName || "No logo chosen"}
          </span>
        </div>
        {#if settings.watermarkPath}
          <p class="text-[11px] text-neutral-500">
            {settings.watermarkSizePercent}% wide · {settings.watermarkOpacityPercent}% opacity ·
            {settings.watermarkPosition.replace("-", " ")}
          </p>
        {:else}
          <!-- Enabled with no logo would otherwise run and quietly change nothing. -->
          <p class="flex items-start gap-1.5 text-[11px] leading-snug text-warn-400">
            <TriangleAlert size={13} class="mt-px shrink-0" />
            No logo chosen, so no watermark will be applied.
          </p>
        {/if}
      </Stage>

      <Stage
        title="Keep metadata"
        icon={Tags}
        bind:enabled={settings.preserveMetadata}
        summary="EXIF, GPS and colour profiles are dropped."
      >
        <p class="text-[11px] leading-snug text-neutral-500">
          Copies EXIF and the ICC colour profile onto the output. The rotation tag is reset,
          because the rotation is already baked into the pixels.
        </p>
      </Stage>
    </section>

    <!-- Queue -->
    <main class="flex min-w-0 flex-1 flex-col">
      <div class="flex h-11 shrink-0 items-center gap-3 border-b border-line px-5 text-xs">
        {#if entries.length > 0}
          <Images size={14} class="text-neutral-500" />
          <span class="font-medium text-neutral-100">{entries.length} images</span>
          <span class="text-neutral-600">{formatBytes(totalInputBytes)}</span>

          <div class="ml-auto flex items-center gap-2">
            <button
              onclick={() => (showCompareDialog = true)}
              disabled={!selectedEntry}
              class="btn h-7"
              title={selectedEntry
                ? `Preview ${selectedEntry.fileName} with these settings`
                : "Select an image first"}
            >
              Compare
            </button>
            <button onclick={addFiles} class="btn h-7">Add files</button>
            <button onclick={() => (confirming = "clear")} class="btn btn-danger h-7">
              <Trash2 size={13} /> Clear all
            </button>
          </div>
        {:else}
          <span class="text-neutral-600">Queue is empty</span>
        {/if}
      </div>

      {#if summary}
        <div
          class="flex shrink-0 flex-wrap items-center gap-x-6 gap-y-1 border-b border-line bg-surface-panel px-5 py-2.5 text-sm"
        >
          <div>
            <span class="text-neutral-500">Processed</span>
            <span class="ml-2 font-medium text-success-400">{summary.processed}</span>
          </div>
          {#if summary.failed > 0}
            <div>
              <span class="text-neutral-500">Failed</span>
              <span class="ml-2 font-medium text-danger-400">{summary.failed}</span>
            </div>
          {/if}
          {#if summary.cancelled}
            <div class="text-warn-400">Stopped early</div>
          {/if}
          <div>
            <span class="text-neutral-500">Size</span>
            <span class="ml-2 tabular-nums">
              {formatBytes(summary.totalOriginalBytes)} → {formatBytes(summary.totalOutputBytes)}
            </span>
          </div>
          <div
            class="ml-auto rounded-full bg-success-500/10 px-3 py-1 text-xs font-medium text-success-400"
          >
            {savingsPercent(summary.totalOriginalBytes, summary.totalOutputBytes)}% smaller
          </div>
        </div>
      {/if}

      {#if errorMessage}
        <div
          class="flex shrink-0 items-start gap-3 border-b border-danger-500/30 bg-danger-500/10 px-5 py-2 text-sm text-danger-400"
        >
          <TriangleAlert size={15} class="mt-0.5 shrink-0" />
          <span class="min-w-0 flex-1">{errorMessage}</span>
          <button
            onclick={() => (errorMessage = null)}
            aria-label="Dismiss"
            class="shrink-0 rounded p-0.5 text-danger-400/70 transition hover:text-danger-400"
          >
            <X size={15} />
          </button>
        </div>
      {/if}

      <div class="min-h-0 flex-1 px-5 pb-3 pt-4">
        {#if entries.length === 0}
          <button
            onclick={chooseInputFolder}
            class="flex h-full w-full flex-col items-center justify-center gap-3 rounded-2xl border-2 border-dashed transition {dragging
              ? 'border-accent-500 bg-accent-500/5'
              : 'border-line hover:border-line-strong'}"
          >
            <UploadCloud size={38} strokeWidth={1.25} class="text-neutral-600" />
            <span class="text-sm text-neutral-200">
              {scanning ? "Scanning…" : "Drop images or a folder here"}
            </span>
            <span class="text-xs text-neutral-500">or click to choose a folder</span>
          </button>
        {:else}
          <div
            class="h-full overflow-hidden rounded-xl border transition {dragging
              ? 'border-accent-500'
              : 'border-line'}"
          >
            <FileList
              rows={entries}
              {selectedPath}
              onselect={(path) => (selectedPath = path)}
              onremove={removeEntry}
              onopen={(path) => {
                selectedPath = path;
                showCompareDialog = true;
              }}
            />
          </div>
        {/if}
      </div>

      <!-- Action bar. Progress sits here rather than at the top of the panel: while a run
           is going, this is where the user is already looking. -->
      <div class="shrink-0 border-t border-line bg-surface-panel px-5 py-4">
        {#if runState !== "idle"}
          <div class="mx-auto mb-3 max-w-2xl">
            <div class="mb-1.5 flex items-baseline justify-between text-xs">
              <span class="uppercase tracking-wide text-neutral-500">
                {runState === "running" ? "Processing" : "Finished"}
              </span>
              <span class="tabular-nums text-neutral-400">
                {completed} / {entries.length} · {progressPercent}%
              </span>
            </div>
            <div class="h-1.5 w-full overflow-hidden rounded-full bg-surface-control">
              <div
                class="h-full rounded-full transition-all duration-200 {runState === 'done'
                  ? 'bg-success-500'
                  : 'bg-accent-500'}"
                style="width: {progressPercent}%"
              ></div>
            </div>
          </div>
        {:else}
          <p class="mb-3 truncate text-center text-[11px]" title={outputFolder}>
            {#if outputFolder}
              <span class="text-neutral-500">{outputFolder}</span>
              <span class="text-neutral-700"> · </span>
              <span class="text-neutral-500">{namePreview}</span>
            {:else if entries.length > 0 && destination.mode === "subfolder"}
              <span class="text-warn-400">
                These images come from different locations — pick another destination folder.
              </span>
            {:else}
              <span class="text-neutral-600">Choose an input folder to set the destination</span>
            {/if}
          </p>
        {/if}

        <div class="flex items-center justify-center gap-3">
          {#if runState === "running"}
            <button onclick={cancelBatch} class="btn h-12 gap-2 px-10 text-[15px]">
              <Square size={15} /> Stop
            </button>
          {:else if runState === "done"}
            <!-- Deliberately not the filled accent: the run is over, so this is a way back
                 to the start rather than the thing to reach for. -->
            <button
              onclick={clearQueue}
              class="btn h-12 gap-2 border-accent-500/40 px-10 text-[15px] text-accent-300 hover:border-accent-500"
            >
              <RotateCcw size={16} /> New batch
            </button>
            <button
              onclick={openOutputFolder}
              disabled={!outputFolder}
              class="btn h-12 gap-2 px-6"
              title={outputFolder ? `Open ${outputFolder}` : "No destination yet"}
            >
              <FolderOpen size={15} /> Open output folder
            </button>
          {:else}
            <button
              onclick={start}
              disabled={!canStart}
              class="btn btn-primary h-13 gap-2.5 px-14 text-base tracking-wide"
            >
              <Play size={18} />
              Process{entries.length > 0 ? ` ${entries.length}` : ""}
            </button>
          {/if}
        </div>
      </div>
    </main>
  </div>
</div>

{#if showWatermarkDialog}
  <WatermarkDialog
    {settings}
    oncancel={() => (showWatermarkDialog = false)}
    onsave={(draft) => {
      settings = { ...settings, ...draft };
      showWatermarkDialog = false;
    }}
  />
{/if}

{#if confirming === "clear"}
  <ConfirmDialog
    title="Clear the queue?"
    message="All {entries.length} images will be removed from the list. Nothing on disk is touched."
    confirmLabel="Clear all"
    destructive
    onconfirm={() => {
      clearQueue();
      confirming = null;
    }}
    oncancel={() => (confirming = null)}
  />
{/if}

{#if confirming === "reset"}
  <ConfirmDialog
    title="Start over?"
    message="Every processing step returns to its default and the queue is emptied. Saved templates and the files on disk are untouched."
    confirmLabel="Reset everything"
    destructive
    onconfirm={() => {
      resetEverything();
      confirming = null;
    }}
    oncancel={() => (confirming = null)}
  />
{/if}

{#if showCompareDialog && selectedEntry}
  <CompareDialog
    path={selectedEntry.path}
    fileName={selectedEntry.fileName}
    options={toProcessOptions(settings)}
    onclose={() => (showCompareDialog = false)}
  />
{/if}
