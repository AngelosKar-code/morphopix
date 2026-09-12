<script lang="ts">
  import { formatBytes, savingsPercent } from "./format";

  export type Row = {
    path: string;
    fileName: string;
    sizeBytes: number;
    status: "idle" | "done" | "error";
    outputBytes: number;
    error: string | null;
  };

  type Props = {
    rows: Row[];
    selectedPath: string | null;
    onselect: (path: string) => void;
    onremove: (path: string) => void;
    /** Double click: the quickest way to inspect one image. */
    onopen: (path: string) => void;
  };

  let { rows, selectedPath, onselect, onremove, onopen }: Props = $props();

  /** Must match the row height in the markup below. */
  const ROW_HEIGHT = 34;
  const OVERSCAN = 8;

  let viewportHeight = $state(0);
  let scrollTop = $state(0);

  // Only the rows within the viewport exist in the DOM, so a 2000 file folder costs the
  // same as a 20 file one.
  const firstVisible = $derived(
    Math.max(0, Math.floor(scrollTop / ROW_HEIGHT) - OVERSCAN),
  );
  const visibleCount = $derived(
    Math.ceil(viewportHeight / ROW_HEIGHT) + OVERSCAN * 2,
  );
  const window = $derived(rows.slice(firstVisible, firstVisible + visibleCount));

  function statusColor(status: Row["status"]) {
    if (status === "done") return "bg-success-500";
    if (status === "error") return "bg-danger-500";
    return "bg-neutral-700";
  }
</script>

<div
  class="h-full overflow-y-auto"
  bind:clientHeight={viewportHeight}
  onscroll={(event) => (scrollTop = event.currentTarget.scrollTop)}
>
  <!-- Spacer gives the scrollbar the full height of the un-rendered list. -->
  <div style="height: {rows.length * ROW_HEIGHT}px; position: relative;">
    <div style="transform: translateY({firstVisible * ROW_HEIGHT}px);">
      {#each window as row (row.path)}
        <div
          style="height: {ROW_HEIGHT}px;"
          class="group flex w-full items-center gap-3 pl-4 pr-2 text-xs transition-colors {row.path ===
          selectedPath
            ? 'bg-accent-500/15 text-accent-300'
            : 'text-neutral-300 hover:bg-neutral-800/50'}"
        >
          <button
            onclick={() => onselect(row.path)}
            ondblclick={() => onopen(row.path)}
            title="Double click to compare"
            class="flex min-w-0 flex-1 items-center gap-3 text-left"
          >
            <span class="h-1.5 w-1.5 shrink-0 rounded-full {statusColor(row.status)}"></span>
            <span class="flex-1 truncate" title={row.error ?? row.fileName}>{row.fileName}</span>

            {#if row.status === "done" && row.outputBytes > 0}
              <span class="shrink-0 tabular-nums text-neutral-500">
                {formatBytes(row.sizeBytes)} → {formatBytes(row.outputBytes)}
              </span>
              <span class="w-12 shrink-0 text-right tabular-nums text-success-500">
                {savingsPercent(row.sizeBytes, row.outputBytes)}%
              </span>
            {:else if row.status === "error"}
              <span class="shrink-0 text-danger-400">failed</span>
              <span class="w-12 shrink-0"></span>
            {:else}
              <span class="shrink-0 tabular-nums text-neutral-500">
                {formatBytes(row.sizeBytes)}
              </span>
              <span class="w-12 shrink-0"></span>
            {/if}
          </button>

          <button
            onclick={() => onremove(row.path)}
            aria-label="Remove {row.fileName}"
            title="Remove from queue"
            class="shrink-0 rounded px-1.5 py-0.5 text-neutral-600 opacity-0 transition group-hover:opacity-100 hover:bg-danger-500/15 hover:text-danger-400"
          >
            ✕
          </button>
        </div>
      {/each}
    </div>
  </div>
</div>
