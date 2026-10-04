<script lang="ts">
  import { t } from '../lib/i18n.svelte';
  import { sync } from '../stores/sync.svelte';
  let { onconnect, onadd, onexport, onimport, onsettings, onsync }: { onconnect: () => void; onadd: () => void; onexport: () => void; onimport: () => void; onsettings: () => void; onsync: () => void } = $props();
  const running = $derived(sync.status?.state === 'running');
</script>

<div class="toolbar">
  <button class="btn primary" onclick={onconnect} title={t('Str_Connect')}>
    <svg width="14" height="14" viewBox="0 0 16 16"><path d="M 3 2 L 13 8 L 3 14 Z" fill="currentColor" /></svg>
    <span>{t('Str_Connect')}</span>
  </button>
  <button class="btn" onclick={onadd}>
    <svg width="14" height="14" viewBox="0 0 14 14"><path d="M 7 1 L 7 13 M 1 7 L 13 7" stroke="currentColor" stroke-width="2" /></svg>
    <span>{t('Str_AddVm')}</span>
  </button>
  <button class="btn" onclick={onexport}>
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M8 11V2M4.5 5.5 8 2l3.5 3.5M2 10v3.5h12V10" /></svg>
    <span>{t('Str_Export')}</span>
  </button>
  <button class="btn" onclick={onimport}>
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6"><path d="M8 2v9M4.5 7.5 8 11l3.5-3.5M2 10v3.5h12V10" /></svg>
    <span>{t('Str_Import')}</span>
  </button>
  <button class="btn" class:err={sync.isError} onclick={onsync} title={sync.statusText || t('Str_Sync')}>
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.6" class:spin={running}><path d="M13.5 8a5.5 5.5 0 0 1-9.6 3.7M2.5 8a5.5 5.5 0 0 1 9.6-3.7" /><path d="M12.5 1.5v3h-3M3.5 14.5v-3h3" /></svg>
    <span>{t('Str_Sync')}</span>
  </button>
  <button class="btn" onclick={onsettings}>
    <svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="currentColor" stroke-width="1.5"><circle cx="8" cy="8" r="2.2" /><path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" /></svg>
    <span>{t('Str_Settings')}</span>
  </button>
</div>

<style>
  .btn.err { color: var(--danger); border-color: var(--danger); }
  .toolbar { display: flex; align-items: center; gap: 8px; padding: 8px 10px; background: var(--theme-toolbar-bg); border-bottom: 1px solid var(--theme-border); }
</style>
