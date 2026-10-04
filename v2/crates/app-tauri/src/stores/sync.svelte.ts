import { invoke, listen } from '../lib/ipc';
import { t } from '../lib/i18n.svelte';
import type { SyncOutcome, SyncStatus } from '../lib/types';
import { ui } from './ui.svelte';

class SyncStore {
  status = $state<SyncStatus | null>(null);

  async init() {
    try { this.status = await invoke<SyncStatus>('sync_status'); } catch { this.status = null; }
    await listen('sync:status', (p) => { this.status = p as SyncStatus; });
  }
  get enabled() { return !!this.status?.enabled; }

  /** Chữ ở góc phải thanh trạng thái. */
  get statusText(): string {
    const s = this.status;
    if (!s || !s.enabled) return '';
    switch (s.state) {
      case 'running': return t('Str_SyncStatusRunning');
      case 'offline': return t('Str_SyncStatusOffline');
      case 'needPassword': return t('Str_SyncStatusNeedPassword');
      case 'error': return t('Str_SyncStatusError', s.lastError ?? '');
      default: return s.lastSyncAt ? t('Str_SyncStatusOk', new Date(s.lastSyncAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })) : t('Str_SyncStatusNever');
    }
  }
  get isError() { return this.status?.state === 'error' || this.status?.state === 'needPassword'; }

  async runNow(showResult = true) {
    try {
      const o = await invoke<SyncOutcome>('sync_run_now');
      if (showResult) await this.showOutcome(o);
    } catch (e) {
      const msg = String(e);
      if (msg === 'needPassword') await this.askPassword();
      else if (msg !== 'busy' && msg !== 'disabled' && showResult) await ui.message(t('Str_Sync'), msg, 'error');
    }
  }
  async showOutcome(o: SyncOutcome) {
    const r = o.report;
    const changed = r.addLocal.length + r.updateLocal.length + r.deleteLocal.length + r.toRemote > 0;
    let text = changed ? t('Str_SyncDone', r.addLocal.length, r.updateLocal.length, r.deleteLocal.length, r.toRemote) : t('Str_SyncNoChange');
    if (r.mergedDuplicates.length) text += '\n' + t('Str_SyncMergedDup', r.mergedDuplicates.join(', '));
    if (o.messages.length) text += '\n' + o.messages.join('\n');
    await ui.message(t('Str_Sync'), text, 'info');
  }
  async askPassword() {
    const pw = await ui.input(t('Str_SyncEnterPassword'), t('Str_SyncPasswordLabel'), '', true);
    if (!pw) return;
    try { const o = await invoke<SyncOutcome>('sync_set_password', { password: pw }); await this.showOutcome(o); }
    catch (e) { if (String(e) === 'needPassword') await ui.message(t('Str_Sync'), t('Str_WrongFilePassword'), 'warn'); else await ui.message(t('Str_Sync'), String(e), 'error'); }
  }
  async changePassword() {
    const pw = await ui.input(t('Str_SyncChangePassword'), t('Str_SyncNewPassword'), '', true);
    if (!pw) return;
    if (pw.length < 8) { await ui.message(t('Str_Error'), t('Str_ExportPassShort'), 'warn'); return; }
    try { await invoke('sync_change_password', { newPassword: pw }); await ui.message(t('Str_Sync'), t('Str_SyncPasswordChanged'), 'info'); }
    catch (e) { await ui.message(t('Str_Sync'), String(e), 'error'); }
  }
  async disable() {
    if (!(await ui.confirm(t('Str_SyncDisconnect'), t('Str_SyncDisconnectConfirm'), 'question'))) return;
    const del = await ui.confirm(t('Str_SyncDisconnect'), t('Str_SyncDeleteVault'), 'warn', t('Str_Yes'), t('Str_No'));
    try { await invoke('sync_disable', { deleteVault: del }); } catch (e) { await ui.message(t('Str_Sync'), String(e), 'error'); }
  }
  async viewLog() {
    const log = await invoke<string>('sync_log').catch(() => '');
    await ui.message(t('Str_SyncLogTitle'), log || '—', 'info');
  }
}
export const sync = new SyncStore();
