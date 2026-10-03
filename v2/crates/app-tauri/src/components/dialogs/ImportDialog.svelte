<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { invoke, isTauri } from '../../lib/ipc';
  import type { ImportFileInfo, ImportResult } from '../../lib/types';
  import { settings } from '../../stores/settings.svelte';
  import { ui } from '../../stores/ui.svelte';
  let { initialPath = null, onclose, onimported }: { initialPath?: string | null; onclose: () => void; onimported: () => void } = $props();
  let info = $state<ImportFileInfo | null>(null);
  let password = $state('');
  let resolution = $state<'skip' | 'overwrite' | 'addcopy'>('skip');
  let busy = $state(false);

  $effect(() => { if (initialPath) load(initialPath); else scanMoba(false); });
  async function load(path: string) {
    try { info = await invoke<ImportFileInfo>('import_inspect', { path }); password = ''; }
    catch (e) { ui.message(t('Str_ErrorReadFile'), String(e), 'error'); }
  }
  async function browse() {
    if (!isTauri) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const f = await open({ title: t('Str_ImportTitle'), multiple: false, defaultPath: settings.value.LastImportFolder || undefined,
      filters: [{ name: t('Str_AllSupported'), extensions: ['snterm', 'mxtsessions', 'ini'] }, { name: t('Str_SessionFiles'), extensions: ['snterm'] }, { name: t('Str_MobaFiles'), extensions: ['mxtsessions', 'ini', 'txt'] }, { name: t('Str_AllFiles'), extensions: ['*'] }] }).catch(() => null);
    if (typeof f === 'string') await load(f);
  }
  async function scanMoba(interactive: boolean) {
    const c = await invoke<string | null>('find_mobaxterm_candidate').catch(() => null);
    if (c) { await load(c); if (interactive && info) ui.message(t('Str_InfoTitle'), `${info.fileName}: ${t('Str_VmCount', info.count)}`, 'info'); }
    else if (interactive) await browse();
  }
  async function doImport() {
    if (!info) { ui.message(t('Str_InfoTitle'), t('Str_NoFileChosen'), 'warn'); return; }
    if (info.protected && !password) { ui.message(t('Str_InfoTitle'), t('Str_NeedPassword'), 'warn'); return; }
    busy = true;
    try {
      const r = await invoke<ImportResult>('import_sessions', { path: info.path, password: info.protected ? password : null, resolution });
      let summary = t('Str_ImportSummary', r.totalInFile, r.importedCount, r.overwrittenCount, r.skippedCount);
      if (r.corruptSecretsCount > 0) summary += t('Str_ImportCorrupt', r.corruptSecretsCount);
      if (r.messages.length) summary += '\n\n' + r.messages.join('\n');
      await ui.message(t('Str_ImportDone'), summary, 'info');
      onimported(); onclose();
    } catch (e) {
      if (String(e) === 'wrong_password') ui.message(t('Str_Error'), t('Str_WrongFilePassword'), 'warn');
      else ui.message(t('Str_Error'), t('Str_ImportError', String(e)), 'error');
    } finally { busy = false; }
  }
  const statusText = (s: string) => s === 'new' ? t('Str_StatusNew') : s === 'duplicate' ? t('Str_StatusDuplicate') : t('Str_StatusInvalid');
</script>

<Modal title={t('Str_ImportTitle')} width={600} onclose={onclose}>
  <div style="display:flex; align-items:center; gap:8px; margin-bottom:10px">
    <div style="flex:1; min-width:0">
      <div style="font-weight:600; overflow:hidden; text-overflow:ellipsis; white-space:nowrap">{info ? `${info.fileName} (${t('Str_VmCount', info.count)})` : t('Str_NoFileChosen')}</div>
      <div class="muted" style="font-size:12px">{info ? (info.format === 'mobaxterm-sessions' ? t('Str_FromMoba') : info.protected ? t('Str_FileProtected') : t('Str_FileNotProtected')) : t('Str_ChooseFileHint')}</div>
    </div>
    <button class="btn" onclick={browse}>{t('Str_BrowseBtn')}</button>
  </div>
  {#if info?.protected}
    <div class="box"><div style="font-weight:600; color:#f04438; margin-bottom:6px">{t('Str_EnterFilePassword')}</div><input class="input" type="password" bind:value={password} onkeydown={(e) => { if (e.key === 'Enter') doImport(); }} /></div>
  {/if}
  {#if info}
    <div class="table">
      <div class="hr"><span>{t('Str_ColName')}</span><span>user@host:port</span><span>{t('Str_GroupLabel')}</span><span>{t('Str_ColStatus')}</span></div>
      {#each info.items as it}
        <div class="tr" class:bad={it.status === 'invalid'}><span title={it.name}>{it.name}</span><span>{it.subtitle}</span><span>{it.group}</span><span class="st {it.status}">{statusText(it.status)}{#if it.existingName} ({it.existingName}){/if}</span></div>
      {/each}
    </div>
  {/if}
  <div style="font-weight:600; margin:10px 0 6px">{t('Str_ConflictHeader')}</div>
  <label class="check" style="margin-bottom:4px"><input type="radio" name="res" value="skip" bind:group={resolution} />{t('Str_ConflictSkip')}</label>
  <label class="check" style="margin-bottom:4px"><input type="radio" name="res" value="overwrite" bind:group={resolution} />{t('Str_ConflictOverwrite')}</label>
  <label class="check"><input type="radio" name="res" value="addcopy" bind:group={resolution} />{t('Str_ConflictAddCopy')}</label>
  {#snippet actions()}
    <button class="btn" onclick={() => scanMoba(true)} style="margin-right:auto" title={t('Str_ScanMobaTooltip')}>{t('Str_ScanMobaBtn')}</button>
    <button class="btn primary" onclick={doImport} disabled={busy || !info}>{t('Str_ImportAction')}</button>
    <button class="btn" onclick={onclose}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>

<style>
  .box { background: var(--theme-sidebar-bg); border: 1px solid var(--theme-border); border-radius: 6px; padding: 10px 12px; margin-bottom: 10px; }
  .table { max-height: 200px; overflow: auto; border: 1px solid var(--theme-border); border-radius: 4px; background: var(--theme-input-bg); font-size: 12px; }
  .hr, .tr { display: grid; grid-template-columns: 1.2fr 1.4fr 0.8fr 1fr; gap: 8px; padding: 4px 8px; }
  .hr { font-weight: 600; color: var(--theme-fg-muted); border-bottom: 1px solid var(--theme-border); }
  .tr span { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .tr.bad { opacity: 0.6; }
  .st.new { color: var(--status-connected); } .st.duplicate { color: var(--warning); } .st.invalid { color: var(--danger); }
</style>
