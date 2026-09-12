<script lang="ts">
  import Modal from "./Modal.svelte";

  type Props = {
    title: string;
    message: string;
    confirmLabel?: string;
    destructive?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  };

  let {
    title,
    message,
    confirmLabel = "Confirm",
    destructive = false,
    onconfirm,
    oncancel,
  }: Props = $props();
</script>

<Modal {title} width="max-w-md" onclose={oncancel}>
  <p class="text-sm leading-relaxed text-neutral-300">{message}</p>

  {#snippet footer()}
    <button onclick={oncancel} class="btn">Cancel</button>
    <button
      onclick={onconfirm}
      class="btn {destructive
        ? 'border-danger-500/40 text-danger-400 hover:border-danger-500'
        : 'btn-primary h-8 px-4 text-[13px]'}"
    >
      {confirmLabel}
    </button>
  {/snippet}
</Modal>
