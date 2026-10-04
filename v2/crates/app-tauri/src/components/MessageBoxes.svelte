<script lang="ts">
  import { ui } from '../stores/ui.svelte';
  import { t } from '../lib/i18n.svelte';
  import Modal from './Modal.svelte';
  let value = $state('');
  let box = $derived(ui.boxes[ui.boxes.length - 1]);
  $effect(() => { if (box?.kind === 'input') value = box.defaultValue ?? ''; });
  const icons: Record<string, string> = { info: 'ℹ️', warn: '⚠️', error: '⛔', question: '❓' };
  function focusEl(el: HTMLElement) { setTimeout(() => { el.focus(); if (el instanceof HTMLInputElement) el.select(); }, 0); }
</script>

{#if box}
  <Modal title={box.title} width={430} onclose={() => ui.close(box, box.kind === 'confirm' ? false : null)}>
    <div style="display:flex; gap:12px; align-items:flex-start;">
      {#if box.icon}<div style="font-size:22px; line-height:1;">{icons[box.icon]}</div>{/if}
      <div style="flex:1; white-space:pre-wrap; user-select:text;">{box.text}{#if box.detail}<div class="muted" style="margin-top:8px; font-size:12px;">{box.detail}</div>{/if}
        {#if box.kind === 'input'}
          <input class="input" style="margin-top:10px" type={box.password ? 'password' : 'text'} bind:value use:focusEl onkeydown={(e) => { if (e.key === 'Enter') ui.close(box, value); }} />
        {/if}
      </div>
    </div>
    {#snippet actions()}
      {#if box.kind === 'message'}
        <button class="btn primary" onclick={() => ui.close(box, true)} use:focusEl>{t('Str_Ok')}</button>
      {:else if box.kind === 'confirm'}
        <button class="btn primary" onclick={() => ui.close(box, true)} use:focusEl>{box.okLabel ?? t('Str_Yes')}</button>
        <button class="btn" onclick={() => ui.close(box, false)}>{box.cancelLabel ?? t('Str_No')}</button>
      {:else}
        <button class="btn primary" onclick={() => ui.close(box, value)}>{t('Str_Ok')}</button>
        <button class="btn" onclick={() => ui.close(box, null)}>{t('Str_Cancel')}</button>
      {/if}
    {/snippet}
  </Modal>
{/if}
