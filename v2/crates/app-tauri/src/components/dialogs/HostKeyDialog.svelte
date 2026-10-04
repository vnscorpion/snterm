<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import type { DialogRequest } from '../../lib/types';
  let { req, onanswer }: { req: DialogRequest; onanswer: (decision: 'trustSave' | 'trustOnce' | 'cancel') => void } = $props();
  function focusEl(el: HTMLElement) { setTimeout(() => el.focus(), 0); }
</script>

<Modal title={req.changed ? t('Str_HostKeyChangedTitle') : t('Str_HostKeyTitle')} width={520} onclose={() => onanswer('cancel')}>
  <div class="title" class:danger={req.changed}>{req.title}</div>
  {#if req.changed}
    <div class="warnbox">
      <div style="font-weight:700; color:#f04438; margin-bottom:4px">{t('Str_HostKeyChangedWarn')}</div>
      <div style="color:#fecdca">{t('Str_HostKeyChangedDetail')}</div>
    </div>
  {:else}
    <div class="muted" style="margin-bottom:10px">{t('Str_HostKeyFirst')}</div>
  {/if}
  <div class="grid">
    <span class="k">{t('Str_Server')}</span><span class="v">{req.host}:{req.port}</span>
    <span class="k">{t('Str_Algorithm')}</span><span class="v">{req.algorithm}</span>
  </div>
  <div class="k" style="margin:8px 0 4px">{t('Str_Fingerprint')}</div>
  <input class="input mono" readonly value={req.fingerprint} />
  {#snippet actions()}
    {#if req.changed}
      <button class="btn danger" onclick={() => onanswer('trustSave')}>{t('Str_ConnectAnyway')}</button>
      <button class="btn" onclick={() => onanswer('cancel')} use:focusEl>{t('Str_Cancel')}</button>
    {:else}
      <button class="btn primary" onclick={() => onanswer('trustSave')} use:focusEl>{t('Str_TrustSave')}</button>
      <button class="btn" onclick={() => onanswer('trustOnce')}>{t('Str_TrustOnce')}</button>
      <button class="btn" onclick={() => onanswer('cancel')}>{t('Str_Cancel')}</button>
    {/if}
  {/snippet}
</Modal>

<style>
  .title { font-weight: 700; font-size: 15px; margin-bottom: 8px; }
  .title.danger { color: #b42318; }
  .warnbox { background: #3d1a1a; border: 1px solid #f04438; border-radius: 6px; padding: 12px; margin-bottom: 10px; }
  .grid { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; }
  .k { font-weight: 600; color: var(--theme-fg-muted); }
  .mono { font-family: Consolas, 'JetBrains Mono', monospace; font-size: 12px; user-select: text; }
</style>
