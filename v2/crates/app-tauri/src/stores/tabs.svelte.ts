import { invoke } from '../lib/ipc';
import type { ConnectionStatus, MonitorInfo, SessionView } from '../lib/types';
import { sessions } from './sessions.svelte';

export interface Tab {
  id: string;            // tab id từ backend (rỗng cho tới khi terminal_open trả về)
  localId: string;       // id phía giao diện (ổn định)
  session: SessionView;
  title: string;
  status: ConnectionStatus;
  monitor: MonitorInfo | null;
  pendingOpen: boolean;
}

class TabsStore {
  list = $state<Tab[]>([]);
  selectedLocalId = $state<string | null>(null);
  leftTab = $state<0 | 1>(0);

  get selected(): Tab | null { return this.list.find((t) => t.localId === this.selectedLocalId) ?? null; }
  byTabId(tabId: string) { return this.list.find((t) => t.id === tabId); }
  byLocalId(localId: string) { return this.list.find((t) => t.localId === localId); }

  private uniqueTitle(base: string): string {
    let name = base; let n = 2;
    while (this.list.some((t) => t.title.toLowerCase() === name.toLowerCase())) name = `${base} (${n++})`;
    return name;
  }

  /** Tạo tab (chưa kết nối; TerminalPane sẽ gọi terminal_open khi xterm sẵn sàng). */
  open(session: SessionView, select = true): Tab {
    const tab: Tab = { id: '', localId: crypto.randomUUID(), session, title: this.uniqueTitle(session.displayName), status: 'connecting', monitor: null, pendingOpen: true };
    this.list = [...this.list, tab];
    if (select) this.selectedLocalId = tab.localId;
    this.refreshLive(session.id);
    return tab;
  }
  openMany(list: SessionView[]) {
    let first: Tab | null = null;
    for (const s of list) { const t = this.open(s, false); first ??= t; }
    if (first) this.selectedLocalId = first.localId;
  }
  select(localId: string | null) {
    const prev = this.selected;
    this.selectedLocalId = localId;
    if (prev?.id) invoke('terminal_set_visible', { tabId: prev.id, visible: false }).catch(() => {});
    const cur = this.selected;
    if (cur?.id) invoke('terminal_set_visible', { tabId: cur.id, visible: true }).catch(() => {});
  }
  setBackendId(localId: string, tabId: string) {
    const t = this.byLocalId(localId); if (t) { t.id = tabId; t.pendingOpen = false; }
  }
  setStatus(tabId: string, status: ConnectionStatus) {
    const t = this.byTabId(tabId); if (!t) return;
    t.status = status;
    this.refreshLive(t.session.id);
  }
  setMonitor(tabId: string, info: MonitorInfo | null) { const t = this.byTabId(tabId); if (t) t.monitor = info; }
  rename(localId: string, title: string) { const t = this.byLocalId(localId); if (t && title.trim()) t.title = title.trim(); }

  refreshLive(sessionId: string) {
    const rel = this.list.filter((t) => t.session.id === sessionId);
    const agg = rel.some((t) => t.status === 'connected') ? 'connected' : rel.some((t) => t.status === 'connecting' || t.status === 'reconnecting') ? 'connecting' : null;
    sessions.setLive(sessionId, agg);
  }

  async close(localId: string) {
    const idx = this.list.findIndex((t) => t.localId === localId);
    if (idx < 0) return;
    const tab = this.list[idx];
    this.list = this.list.filter((t) => t.localId !== localId);
    if (tab.id) invoke('terminal_close', { tabId: tab.id }).catch(() => {});
    if (this.selectedLocalId === localId) {
      const next = this.list[Math.min(idx, this.list.length - 1)];
      this.select(next?.localId ?? null);
    }
    this.refreshLive(tab.session.id);
  }
  async closeOthers(localId: string) { for (const t of [...this.list]) if (t.localId !== localId) await this.close(t.localId); this.select(localId); }
  async closeRight(localId: string) { const idx = this.list.findIndex((t) => t.localId === localId); for (const t of this.list.slice(idx + 1)) await this.close(t.localId); }
  async closeAll() { for (const t of [...this.list]) await this.close(t.localId); }
  move(from: number, to: number) {
    if (from === to || from < 0 || to < 0 || from >= this.list.length || to >= this.list.length) return;
    const l = [...this.list]; const [it] = l.splice(from, 1); l.splice(to, 0, it); this.list = l;
  }
  selectNext() { if (this.list.length <= 1 || !this.selected) return; const i = this.list.indexOf(this.selected); this.select(this.list[(i + 1) % this.list.length].localId); }
  selectPrev() { if (this.list.length <= 1 || !this.selected) return; const i = this.list.indexOf(this.selected); this.select(this.list[(i - 1 + this.list.length) % this.list.length].localId); }
  selectIndex(i: number) { if (i >= 0 && i < this.list.length) this.select(this.list[i].localId); }
  get connectedCount() { return this.list.filter((t) => t.status === 'connected').length; }
}
export const tabs = new TabsStore();
