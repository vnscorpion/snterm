<script lang="ts">
  // Form Thêm/Sửa VM (SessionEditorDialog v1).
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { invoke, isTauri } from '../../lib/ipc';
  import type { SessionDraft, SessionView } from '../../lib/types';
  import { sessions } from '../../stores/sessions.svelte';
  import { ui } from '../../stores/ui.svelte';

  let { editing = null, initialGroup = '', onsaved, oncancel }: { editing?: SessionView | null; initialGroup?: string; onsaved: (s: SessionView, connect: boolean) => void; oncancel: () => void } = $props();

  let name = $state(editing?.name ?? '');
  let group = $state(editing?.group ?? initialGroup);
  let host = $state(editing?.host ?? '');
  let port = $state(String(editing?.port ?? 22));
  let username = $state(editing?.username ?? '');
  let password = $state('');
  let showPw = $state(false);
  let savePassword = $state(editing?.savePassword ?? true);
  let hasStored = $state(editing?.hasPassword ?? false);
  let cleared = $state(false);
  let keyFilePath = $state(editing?.keyFilePath ?? '');
  let passphrase = $state('');
  let testing = $state(false);
  let touched = $state(false);

  const hostError = $derived(touched && !host.trim() ? t('Str_HostError') : null);
  const portNum = $derived(parseInt(port, 10));
  const portError = $derived(touched && (!Number.isInteger(portNum) || portNum < 1 || portNum > 65535) ? t('Str_PortError') : null);
  const userError = $derived(touched && !username.trim() ? t('Str_UserError') : null);
  let keyWarning = $state<string | null>(null);
  $effect(() => {
    const p = keyFilePath.trim();
    if (!p) { keyWarning = null; return; }
    if (!isTauri) { keyWarning = null; return; }
    invoke<string>('inspect_key_file', { path: p, passphrase: passphrase || null }).then(() => (keyWarning = null)).catch((e) => {
      keyWarning = e === 'notfound' ? t('Str_MissingKeyTooltip') : e === 'passphrase' ? t('Str_Passphrase_Required') : t('Str_KeyFileInvalid');
    });
  });

  function validate(): boolean { touched = true; return !!host.trim() && Number.isInteger(portNum) && portNum >= 1 && portNum <= 65535 && !!username.trim(); }
  function draft(): SessionDraft {
    return { id: editing?.id ?? null, name: name.trim(), group: group.trim(), host: host.trim(), port: portNum, username: username.trim(), savePassword,
      password: password || null, clearPassword: cleared, keyFilePath: keyFilePath.trim() || null, passphrase: passphrase || null };
  }
  async function save(connect: boolean) {
    if (!validate()) return;
    try { const s = await sessions.save(draft()); onsaved(s, connect); }
    catch (e) { ui.message(t('Str_Error'), String(e), 'error'); }
  }
  async function test() {
    if (!validate()) return;
    testing = true;
    try { await invoke('test_connection', { draft: draft() }); await ui.message(t('Str_TestConnection'), t('Str_TestConnectionOk'), 'info'); }
    catch (e) { await ui.message(t('Str_TestConnectionFail'), String(e), 'warn'); }
    finally { testing = false; }
  }
  async function browseKey() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const f = await open({ title: t('Str_BrowseKeyFile'), multiple: false, filters: [{ name: t('Str_KeyFilter'), extensions: ['pem', 'key', 'ppk', '*'] }] });
      if (typeof f === 'string') keyFilePath = f;
    } catch {}
  }
  function focusEl(el: HTMLElement) { setTimeout(() => el.focus(), 0); }
</script>

<Modal title={editing ? t('Str_EditVmTitle') : t('Str_AddVmTitle')} width={520} onclose={oncancel}>
  <label class="form-label" for="f-name">{t('Str_VmNameLabel')}</label>
  <input id="f-name" class="input" bind:value={name} style="margin-bottom:6px" use:focusEl />
  <label class="form-label" for="f-group">{t('Str_GroupLabel')}</label>
  <input id="f-group" class="input" bind:value={group} list="groups" placeholder={t('Str_PlaceholderGroup')} style="margin-bottom:6px" />
  <datalist id="groups">{#each sessions.existingGroups as g}<option value={g}></option>{/each}</datalist>
  <label class="form-label required" for="f-host">{t('Str_HostLabel')}</label>
  <input id="f-host" class="input" class:error={!!hostError} bind:value={host} oninput={() => (touched = true)} />
  {#if hostError}<div class="field-error">{hostError}</div>{:else}<div style="height:6px"></div>{/if}
  <div style="display:grid; grid-template-columns: 110px 1fr; gap: 10px">
    <div>
      <label class="form-label required" for="f-port">{t('Str_PortLabel')}</label>
      <input id="f-port" class="input" class:error={!!portError} bind:value={port} maxlength="5" oninput={() => (touched = true)} />
      {#if portError}<div class="field-error">{portError}</div>{/if}
    </div>
    <div>
      <label class="form-label required" for="f-user">{t('Str_UserLabel')}</label>
      <input id="f-user" class="input" class:error={!!userError} bind:value={username} oninput={() => (touched = true)} />
      {#if userError}<div class="field-error">{userError}</div>{/if}
    </div>
  </div>
  <div class="section-title" style="margin-top:12px">{t('Str_AuthLabel')}</div>
  <label class="form-label" for="f-pw">{t('Str_PasswordLabel')}</label>
  <div style="display:flex; gap:4px">
    <input id="f-pw" class="input" type={showPw ? 'text' : 'password'} bind:value={password} autocomplete="off" />
    <button class="btn" style="width:34px; padding:0" onclick={() => (showPw = !showPw)} title="👁">👁</button>
  </div>
  {#if hasStored && !password}
    <div style="display:flex; align-items:center; gap:8px; margin:4px 0 6px">
      <span class="muted" style="font-size:11px">{t('Str_UsingSavedPassword')}</span>
      <button class="btn link" style="font-size:11px" onclick={() => { cleared = true; hasStored = false; password = ''; }}>{t('Str_ClearSavedPassword')}</button>
    </div>
  {:else}<div style="height:6px"></div>{/if}
  <label class="check" style="margin-bottom:8px"><input type="checkbox" bind:checked={savePassword} />{t('Str_SavePassword')}</label>
  <label class="form-label" for="f-key">{t('Str_KeyFileLabel')}</label>
  <div style="display:flex; gap:4px">
    <input id="f-key" class="input" bind:value={keyFilePath} />
    <button class="btn" onclick={browseKey}>{t('Str_BrowseBtn')}</button>
  </div>
  {#if keyWarning}<div class="field-warn">{keyWarning}</div>{:else}<div style="height:6px"></div>{/if}
  <label class="form-label" for="f-pp">{t('Str_KeyPassphraseLabel')}</label>
  <input id="f-pp" class="input" type="password" bind:value={passphrase} autocomplete="off" disabled={!keyFilePath.trim()} />
  {#snippet actions()}
    <button class="btn" onclick={test} disabled={testing} style="margin-right:auto">{#if testing}<span class="spin">⟳</span> {t('Str_Testing')}{:else}{t('Str_TestConnection')}{/if}</button>
    <button class="btn primary" onclick={() => save(true)}>{t('Str_SaveAndConnect')}</button>
    <button class="btn" onclick={() => save(false)}>{t('Str_Save')}</button>
    <button class="btn" onclick={oncancel}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>
