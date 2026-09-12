<script lang="ts">
  import type { Snippet } from "svelte";

  type Props = {
    title: string;
    subtitle?: string;
    /** Tailwind max-width class; compare needs far more room than a settings sheet. */
    width?: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
  };

  let { title, subtitle = "", width = "max-w-lg", onclose, children, footer }: Props =
    $props();

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "Escape") onclose();
  }
</script>

<svelte:window onkeydown={onKeyDown} />

<!-- Backdrop. Clicking outside the panel dismisses, as with any lightbox. -->
<div
  class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-6 backdrop-blur-sm"
  role="presentation"
  onclick={(event) => event.target === event.currentTarget && onclose()}
>
  <div
    role="dialog"
    aria-modal="true"
    aria-label={title}
    class="flex h-auto max-h-full w-full {width} flex-col overflow-hidden rounded-2xl border border-line bg-surface-panel shadow-2xl"
  >
    <header class="flex items-start justify-between gap-4 border-b border-line px-5 py-3.5">
      <div class="min-w-0">
        <h2 class="text-sm font-semibold text-neutral-100">{title}</h2>
        {#if subtitle}
          <p class="mt-0.5 truncate text-xs text-neutral-500">{subtitle}</p>
        {/if}
      </div>
      <button onclick={onclose} aria-label="Close" class="btn h-7 shrink-0 px-2">✕</button>
    </header>

    <div class="min-h-0 flex-1 overflow-y-auto p-5">
      {@render children()}
    </div>

    {#if footer}
      <footer class="flex items-center justify-end gap-2 border-t border-line px-5 py-3">
        {@render footer()}
      </footer>
    {/if}
  </div>
</div>
