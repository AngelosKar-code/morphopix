<script lang="ts">
  import type { Component, Snippet } from "svelte";

  type Props = {
    title: string;
    icon?: Component<{ size?: number; class?: string }>;
    enabled?: boolean;
    /** Rendered instead of the body while the stage is switched off. */
    summary?: string;
    children?: Snippet;
    /** A secondary control in the header, e.g. "Configure…". */
    action?: Snippet;
  };

  let {
    title,
    icon: Icon,
    enabled = $bindable(true),
    summary = "",
    children,
    action,
  }: Props = $props();
</script>

<!--
  `shrink-0` is load bearing: as a flex child this card would otherwise be compressed by the
  column instead of pushing the column into scrolling, which silently clipped the contents
  of whichever card was open.
-->
<section class="card shrink-0 overflow-hidden {enabled ? '' : 'opacity-80'}">
  <header class="flex items-center gap-2.5 px-3.5 py-2.5">
    {#if Icon}
      <Icon size={15} class={enabled ? "text-accent-400" : "text-neutral-600"} />
    {/if}

    <span
      class="flex-1 text-[11px] font-semibold uppercase tracking-wide {enabled
        ? 'text-neutral-100'
        : 'text-neutral-500'}"
    >
      {title}
    </span>

    {#if action}
      {@render action()}
    {/if}

    <input
      type="checkbox"
      bind:checked={enabled}
      aria-label="Enable {title}"
      class="accent-accent-500"
    />
  </header>

  {#if enabled && children}
    <div class="space-y-3 border-t border-line px-3.5 py-3">
      {@render children()}
    </div>
  {:else if !enabled && summary}
    <p class="border-t border-line px-3.5 py-2.5 text-[11px] text-neutral-500">
      {summary}
    </p>
  {/if}
</section>
