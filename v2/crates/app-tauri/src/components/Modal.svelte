<script lang="ts">
  import type { Snippet } from 'svelte';
  let { title, width = 460, onclose, children, actions }: { title: string; width?: number; onclose?: () => void; children: Snippet; actions?: Snippet } = $props();
  let root: HTMLDivElement | undefined = $state();
  // Esc đóng hộp thoại trên cùng (bắt ở mức window để không phụ thuộc focus).
  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      const all = document.querySelectorAll('.modal-backdrop');
      if (all[all.length - 1] === root) { e.stopPropagation(); e.preventDefault(); onclose?.(); }
    };
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="modal-backdrop" bind:this={root} onmousedown={(e) => { if (e.target === e.currentTarget) onclose?.(); }}>
  <div class="modal" style="width:{width}px" role="dialog" aria-modal="true" tabindex="-1">
    <div class="modal-title">{title}</div>
    <div class="modal-body">{@render children()}</div>
    {#if actions}<div class="modal-actions">{@render actions()}</div>{/if}
  </div>
</div>
