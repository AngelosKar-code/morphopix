<script lang="ts">
  type Props = {
    beforeUri: string;
    afterUri: string;
    stale?: boolean;
  };

  let { beforeUri, afterUri, stale = false }: Props = $props();

  let container = $state<HTMLDivElement | null>(null);
  let width = $state(0);
  let position = $state(50);
  let dragging = $state(false);

  function setFromPointer(clientX: number) {
    if (!container) return;
    const box = container.getBoundingClientRect();
    position = Math.min(100, Math.max(0, ((clientX - box.left) / box.width) * 100));
  }

  function onPointerDown(event: PointerEvent) {
    dragging = true;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    setFromPointer(event.clientX);
  }

  function onPointerMove(event: PointerEvent) {
    if (dragging) setFromPointer(event.clientX);
  }

  function onKeyDown(event: KeyboardEvent) {
    if (event.key === "ArrowLeft") position = Math.max(0, position - 2);
    if (event.key === "ArrowRight") position = Math.min(100, position + 2);
  }
</script>

<div
  bind:this={container}
  bind:clientWidth={width}
  role="slider"
  tabindex="0"
  aria-label="Compare original and output"
  aria-valuenow={Math.round(position)}
  aria-valuemin="0"
  aria-valuemax="100"
  class="relative select-none overflow-hidden rounded-xl border border-neutral-800 bg-neutral-900 {stale
    ? 'opacity-60'
    : ''}"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={() => (dragging = false)}
  onpointercancel={() => (dragging = false)}
  onkeydown={onKeyDown}
>
  <!-- Output fills the frame; the original is clipped over it. -->
  <img src={afterUri} alt="Processed output" class="block w-full" draggable="false" />

  <div class="absolute inset-y-0 left-0 overflow-hidden" style="width: {position}%">
    <!-- Fixed pixel width keeps the two images in register while the clip moves. -->
    <img
      src={beforeUri}
      alt="Original"
      class="block max-w-none"
      style="width: {width}px"
      draggable="false"
    />
  </div>

  <div
    class="pointer-events-none absolute inset-y-0 w-0.5 bg-white/80"
    style="left: {position}%"
  >
    <div
      class="absolute top-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full bg-white px-2 py-1 text-[10px] font-medium text-neutral-900 shadow"
    >
      ◄ ►
    </div>
  </div>

  <span
    class="pointer-events-none absolute left-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white"
  >
    Original
  </span>
  <span
    class="pointer-events-none absolute right-2 top-2 rounded bg-black/60 px-1.5 py-0.5 text-[10px] text-white"
  >
    Output
  </span>
</div>
