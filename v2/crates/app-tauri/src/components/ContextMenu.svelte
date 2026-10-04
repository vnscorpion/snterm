<script lang="ts">
  import { ui } from '../stores/ui.svelte';
  let el: HTMLDivElement | undefined = $state();
  let pos = $derived.by(() => {
    if (!ui.menu) return { x: 0, y: 0 };
    const w = el?.offsetWidth ?? 200, h = el?.offsetHeight ?? 200;
    return { x: Math.max(0, Math.min(ui.menu.x, window.innerWidth - w - 4)), y: Math.max(0, Math.min(ui.menu.y, window.innerHeight - h - 4)) };
  });
  $effect(() => {
    if (!ui.menu) return;
    const close = () => ui.closeMenu();
    const onKey = (e: KeyboardEvent) => { if (e.key === 'Escape') close(); };
    window.addEventListener('mousedown', close, true);
    window.addEventListener('blur', close);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('resize', close);
    return () => { window.removeEventListener('mousedown', close, true); window.removeEventListener('blur', close); window.removeEventListener('keydown', onKey, true); window.removeEventListener('resize', close); };
  });
</script>

{#if ui.menu}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="ctx-menu" bind:this={el} style="left:{pos.x}px; top:{pos.y}px" onmousedown={(e) => e.stopPropagation()}>
    {#each ui.menu.items as item}
      {#if item.sep}
        <div class="sep"></div>
      {:else}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="item" class:disabled={item.disabled} style={item.danger ? 'color:var(--danger)' : ''} onclick={() => { ui.closeMenu(); item.action?.(); }}>
          <span>{item.label}</span>{#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
        </div>
      {/if}
    {/each}
  </div>
{/if}
