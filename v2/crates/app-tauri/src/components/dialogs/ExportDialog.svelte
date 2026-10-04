<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { invoke, isTauri } from '../../lib/ipc';
  import { passwordStrength } from '../../lib/format';
  import type { ExportOutcome, SessionView } from '../../lib/types';
  import { sessions } from '../../stores/sessions.svelte';
  import { settings } from '../../stores/settings.svelte';
  import { ui } from '../../stores/ui.svelte';
  let { preselected = null, onclose }: { preselected?: SessionView[] | null; onclose: () => void } = $props();
  let checked = $state<Set<string>>(new Set((preselected && preselected.length ? preselected : sessions.list).map((s) => s.id)));
  let withPassword = $state(false);
  let pw1 = $state(''); let pw2 = $state(''); let includeKey = $state(true);
  const strength = $derived(passwordStrength(pw1));
  const strengthLabel = $derived([t('Str_PwWeak'), t('Str_PwMedium'), t('Str_PwStrong')][strength]);
  const groups = $derived.by(() => { const m = new Map<string, SessionView[]>(); for (const s of [...sessions.list].sort((a, b) => a.displayName.localeCompare(b.displayName))) { const g = s.effectiveGroup; if (!m.has(g)) m.set(g, []); m.get(g)!.push(s); } return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0])); });
  function toggle(id: string, on: boolean) { const n = new Set(checked); if (on) n.add(id); else n.delete(id); checked = n; }
  function toggleGroup(list: SessionView[], on: boolean) { const n = new Set(checked); for (const s of list) { if (on) n.add(s.id); else n.delete(s.id); } checked = n; }
  async function doExport() {
    const ids = [...checked];
    if (!ids.length) { ui.message(t('Str_InfoTitle'), t('Str_ExportNeedSelect'), 'warn'); return; }
    let password: string | null = null;
    if (withPassword) {
      if (pw1.length < 8) { ui.message(t('Str_Error'), t('Str_ExportPassShort'), 'warn'); return; }
      if (pw1 !== pw2) { ui.message(t('Str_Error'), t('Str_ExportPassMismatch'), 'warn'); return; }
      password = pw1;
    }
    const d = new Date(); const p = (n: number) => String(n).padStart(2, '0');
    const def = `snterm_backup_${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}.snterm`;
    let path: string | null = null;
    if (isTauri) {
      const { save } = await import('@tauri-apps/plugin-dialog');
      path = await save({ title: t('Str_SaveFileTitle'), defaultPath: settings.value.LastExportFolder ? `${settings.value.LastExportFolder}/${def}` : def, filters: [{ name: t('Str_SessionFiles'), extensions: ['snterm'] }] }).catch(() => null);
    } else path = def;
    if (!path) return;
    try {
      const r = await invoke<ExportOutcome>('export_sessions', { ids, path, password, includeKeyContent: withPassword && includeKey });
      await ui.message(t('Str_InfoTitle'), t('Str_ExportDone', r.exported) + (r.messages.length ? '\n' + r.messages.join('\n') : ''), 'info');
      onclose();
    } catch (e) { ui.message(t('Str_Error'), t('Str_ExportError', String(e)), 'error'); }
  }
</script>

<Modal title={t('Str_ExportTitle')} width={520} onclose={onclose}>
  <div style="font-weight:600; margin-bottom:8px">{t('Str_ExportSelectPrompt')}</div>
  <div class="list">
    {#each groups as [g, list]}
      <label class="check grp"><input type="checkbox" checked={list.every((s) => checked.has(s.id))} onchange={(e) => toggleGroup(list, (e.currentTarget as HTMLInputElement).checked)} /><b>▾ {g}</b></label>
      {#each list as s}
        <label class="check row"><input type="checkbox" checked={checked.has(s.id)} onchange={(e) => toggle(s.id, (e.currentTarget as HTMLInputElement).checked)} /><span style="font-weight:600">{s.displayName}</span><span class="muted" style="font-size:11px">{s.subtitle}</span></label>
      {/each}
    {/each}
  </div>
  <div style="display:flex; gap:6px; margin:6px 0 12px"><button class="btn small" onclick={() => (checked = new Set(sessions.list.map((s) => s.id)))}>{t('Str_SelectAll')}</button><button class="btn small" onclick={() => (checked = new Set())}>{t('Str_DeselectAll')}</button></div>
  <div class="section-title">{t('Str_PasswordLabel')}</div>
  <label class="check" style="margin-bottom:6px"><input type="radio" name="mode" checked={!withPassword} onchange={() => (withPassword = false)} />{t('Str_NoPasswordExport')}</label>
  <label class="check" style="margin-bottom:8px"><input type="radio" name="mode" checked={withPassword} onchange={() => (withPassword = true)} />{t('Str_WithPasswordExport')}</label>
  <div style="margin-left:18px; opacity:{withPassword ? 1 : 0.5}; pointer-events:{withPassword ? 'auto' : 'none'}">
    <div class="muted" style="font-size:11px">{t('Str_ExportPasswordPrompt')}</div>
    <div style="display:flex; gap:8px; align-items:center; margin-bottom:6px"><input class="input" type="password" bind:value={pw1} /><span class="strength s{strength}">{pw1 ? strengthLabel : ''}</span></div>
    <div class="muted" style="font-size:11px">{t('Str_ConfirmPasswordPrompt')}</div>
    <input class="input" type="password" bind:value={pw2} style="margin-bottom:6px" />
    <label class="check" style="font-size:11px"><input type="checkbox" bind:checked={includeKey} />{t('Str_IncludeKeyFile')}</label>
    <div style="font-size:11px; color:var(--warning); margin-top:6px">{t('Str_ExportRemember')}</div>
  </div>
  {#snippet actions()}
    <button class="btn primary" onclick={doExport} disabled={checked.size === 0}>{t('Str_ExportAction')}</button>
    <button class="btn" onclick={onclose}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>

<style>
  .list { max-height: 220px; overflow: auto; border: 1px solid var(--theme-border); border-radius: 4px; padding: 4px 8px; background: var(--theme-input-bg); }
  .grp { display: flex; margin-top: 4px; }
  .row { display: flex; margin-left: 18px; gap: 8px; padding: 2px 0; }
  .strength { font-size: 11px; min-width: 70px; }
  .s0 { color: var(--danger); } .s1 { color: var(--warning); } .s2 { color: var(--status-connected); }
</style>
