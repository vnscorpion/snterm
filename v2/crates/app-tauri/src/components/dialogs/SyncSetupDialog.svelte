<script lang="ts">
  // Thiết lập đồng bộ 3 bước (Phần 2 mục 7): chọn kho → mật khẩu → xem trước lần gộp đầu.
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { invoke, isTauri } from '../../lib/ipc';
  import { passwordStrength } from '../../lib/format';
  import type { SyncBackendConfig, SyncOutcome, SyncPreview, SyncTestResult } from '../../lib/types';
  import { sessions } from '../../stores/sessions.svelte';
  import { sync } from '../../stores/sync.svelte';
  import { ui } from '../../stores/ui.svelte';

  let { onclose }: { onclose: () => void } = $props();
  let step = $state(1);
  let backendType = $state<'Folder' | 'Sftp'>('Folder');
  let folderPath = $state('');
  let sftpSessionId = $state<string>(sessions.list[0]?.id ?? '');
  let sftpRemotePath = $state('~/.snterm/sync.vault');
  let testing = $state(false);
  let testResult = $state<SyncTestResult | null>(null);
  let pw1 = $state(''); let pw2 = $state('');
  let includeKeys = $state(true);
  let preview = $state<SyncPreview | null>(null);
  let busy = $state(false);
  const strength = $derived(passwordStrength(pw1));
  const strengthLabel = $derived([t('Str_PwWeak'), t('Str_PwMedium'), t('Str_PwStrong')][strength]);
  const config = $derived<SyncBackendConfig>({ backendType, folderPath: folderPath.trim(), sftpSessionId: backendType === 'Sftp' ? sftpSessionId || null : null, sftpRemotePath: sftpRemotePath.trim() });
  const canNext1 = $derived(!!testResult && !testing);

  async function browseFolder() {
    if (!isTauri) { folderPath = 'C:\\Users\\An\\OneDrive\\SNTerm'; return; }
    const { open } = await import('@tauri-apps/plugin-dialog');
    const d = await open({ title: t('Str_SyncChooseFolder'), directory: true }).catch(() => null);
    if (typeof d === 'string') { folderPath = d; testResult = null; }
  }
  async function test() {
    testing = true; testResult = null;
    try { testResult = await invoke<SyncTestResult>('sync_test_backend', { config }); }
    catch (e) { await ui.message(t('Str_Sync'), t('Str_SyncTestFail', String(e)), 'warn'); }
    finally { testing = false; }
  }
  async function next2() {
    if (!testResult) { await test(); if (!testResult) return; }
    step = 2;
  }
  async function next3() {
    if (pw1.length < 8) { await ui.message(t('Str_Error'), t('Str_ExportPassShort'), 'warn'); return; }
    if (!testResult?.vaultExists && pw1 !== pw2) { await ui.message(t('Str_Error'), t('Str_ExportPassMismatch'), 'warn'); return; }
    busy = true;
    try { preview = await invoke<SyncPreview>('sync_preview', { config, password: pw1, includeKeyFiles: includeKeys }); step = 3; }
    catch (e) { await ui.message(t('Str_Sync'), String(e) === 'wrong_password' ? t('Str_WrongFilePassword') : String(e), 'warn'); }
    finally { busy = false; }
  }
  async function start() {
    busy = true;
    try {
      const o = await invoke<SyncOutcome>('sync_enable', { config, password: pw1, includeKeyFiles: includeKeys });
      onclose();
      await sync.showOutcome(o);
      await sessions.load();
    } catch (e) { await ui.message(t('Str_Sync'), String(e), 'error'); }
    finally { busy = false; }
  }
</script>

<Modal title={t('Str_SyncSetupTitle')} width={560} onclose={onclose}>
  {#if step === 1}
    <div class="muted" style="margin-bottom:10px">{t('Str_SyncIntro')}</div>
    <div class="section-title">{t('Str_SyncStep1')}</div>
    <label class="check" style="margin-bottom:6px"><input type="radio" name="bk" checked={backendType === 'Folder'} onchange={() => { backendType = 'Folder'; testResult = null; }} />{t('Str_SyncBackendFolder')}</label>
    {#if backendType === 'Folder'}
      <div class="sub"><label class="form-label" for="sy-folder">{t('Str_SyncFolderLabel')}</label>
        <div style="display:flex; gap:4px"><input id="sy-folder" class="input" bind:value={folderPath} oninput={() => (testResult = null)} /><button class="btn" onclick={browseFolder}>{t('Str_BrowseBtn')}</button></div></div>
    {/if}
    <label class="check" style="margin:8px 0 6px"><input type="radio" name="bk" checked={backendType === 'Sftp'} onchange={() => { backendType = 'Sftp'; testResult = null; }} />{t('Str_SyncBackendSftp')}</label>
    {#if backendType === 'Sftp'}
      <div class="sub">
        <label class="form-label" for="sy-vm">{t('Str_SyncVmLabel')}</label>
        <select id="sy-vm" class="input" bind:value={sftpSessionId} onchange={() => (testResult = null)} style="margin-bottom:6px">
          {#each sessions.list as s}<option value={s.id}>{s.displayName} ({s.subtitle})</option>{/each}
        </select>
        <label class="form-label" for="sy-path">{t('Str_SyncRemotePathLabel')}</label>
        <input id="sy-path" class="input" bind:value={sftpRemotePath} oninput={() => (testResult = null)} />
      </div>
    {/if}
    <div style="display:flex; align-items:center; gap:10px; margin-top:12px">
      <button class="btn" onclick={test} disabled={testing}>{#if testing}<span class="spin">⟳</span>{/if} {t('Str_SyncTest')}</button>
      {#if testResult}<span style="color:var(--status-connected)">{t('Str_SyncTestOk')}</span>{/if}
    </div>
  {:else if step === 2}
    <div class="section-title">{t('Str_SyncStep2')}</div>
    <div style="margin-bottom:8px">{testResult?.vaultExists ? t('Str_SyncVaultExists') : t('Str_SyncVaultNew')}</div>
    <label class="form-label" for="sy-pw">{t('Str_SyncPasswordLabel')}</label>
    <div style="display:flex; gap:8px; align-items:center; margin-bottom:6px"><input id="sy-pw" class="input" type="password" bind:value={pw1} /><span class="strength s{strength}">{pw1 ? strengthLabel : ''}</span></div>
    {#if !testResult?.vaultExists}
      <label class="form-label" for="sy-pw2">{t('Str_ConfirmPasswordPrompt')}</label>
      <input id="sy-pw2" class="input" type="password" bind:value={pw2} style="margin-bottom:8px" />
    {/if}
    <label class="check" style="margin:6px 0"><input type="checkbox" bind:checked={includeKeys} />{t('Str_SyncIncludeKeys')}</label>
    <div style="font-size:11px; color:var(--warning); margin-top:8px">{t('Str_SyncWarn')}</div>
  {:else if preview}
    <div class="section-title">{t('Str_SyncStep3')}</div>
    <div style="margin-bottom:8px">{preview.remoteExisted ? t('Str_SyncPreviewSummary', preview.report.addLocal.length, preview.report.updateLocal.length, preview.report.deleteLocal.length, preview.report.toRemote) : t('Str_SyncPreviewFirstVault', preview.localCount)}</div>
    {#if preview.report.addLocal.length || preview.report.updateLocal.length || preview.report.deleteLocal.length}
      <div class="list">
        {#each preview.report.addLocal as n}<div><span class="st new">+</span> {n}</div>{/each}
        {#each preview.report.updateLocal as n}<div><span class="st upd">↻</span> {n}</div>{/each}
        {#each preview.report.deleteLocal as n}<div><span class="st del">−</span> {n}</div>{/each}
      </div>
    {/if}
    {#if preview.report.mergedDuplicates.length}<div class="muted" style="margin-top:6px">{t('Str_SyncMergedDup', preview.report.mergedDuplicates.join(', '))}</div>{/if}
    {#each preview.messages as m}<div class="muted" style="margin-top:4px">{m}</div>{/each}
  {/if}
  {#snippet actions()}
    {#if step > 1}<button class="btn" onclick={() => (step -= 1)} disabled={busy} style="margin-right:auto">{t('Str_SyncBack')}</button>{/if}
    {#if step === 1}<button class="btn primary" onclick={next2} disabled={testing || (backendType === 'Folder' ? !folderPath.trim() : !sftpSessionId)}>{t('Str_SyncNext')}</button>
    {:else if step === 2}<button class="btn primary" onclick={next3} disabled={busy}>{#if busy}<span class="spin">⟳</span>{/if} {t('Str_SyncNext')}</button>
    {:else}<button class="btn primary" onclick={start} disabled={busy}>{#if busy}<span class="spin">⟳</span>{/if} {t('Str_SyncStart')}</button>{/if}
    <button class="btn" onclick={onclose} disabled={busy}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>

<style>
  .sub { margin-left: 22px; }
  .strength { font-size: 11px; min-width: 70px; }
  .s0 { color: var(--danger); } .s1 { color: var(--warning); } .s2 { color: var(--status-connected); }
  .list { max-height: 200px; overflow: auto; border: 1px solid var(--theme-border); border-radius: 4px; padding: 6px 8px; background: var(--theme-input-bg); font-size: 12px; }
  .st { display: inline-block; width: 14px; font-weight: 700; }
  .st.new { color: var(--status-connected); } .st.upd { color: var(--warning); } .st.del { color: var(--danger); }
</style>
