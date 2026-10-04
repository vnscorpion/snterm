import { invoke, listen } from '../lib/ipc';
import type { SessionDraft, SessionView } from '../lib/types';
import { t } from '../lib/i18n.svelte';
import { settings } from './settings.svelte';

export const RECENT_GROUP = '__recent__';

export interface GroupRow { key: string; title: string; isRecent: boolean; sessions: SessionView[]; }

class SessionsStore {
  list = $state<SessionView[]>([]);
  search = $state('');
  selectedIds = $state<string[]>([]);
  anchorId = $state<string | null>(null);
  collapsed = $state<Set<string>>(new Set());
  recovered = $state(false);
  /** Trạng thái sống của VM (tổng hợp từ các tab): id → 'connected' | 'connecting' */
  live = $state<Record<string, 'connected' | 'connecting'>>({});

  async load() {
    const r = await invoke<{ sessions: SessionView[]; recoveredFromCorruption: boolean }>('list_sessions');
    this.list = r.sessions;
    this.recovered = r.recoveredFromCorruption;
    this.collapsed = new Set(settings.value.CollapsedGroups ?? []);
    this.selectedIds = this.selectedIds.filter((id) => this.list.some((s) => s.id === id));
  }
  async init() {
    await this.load();
    await listen('sessions:changed', () => this.load());
  }

  filtered = $derived.by(() => {
    const q = this.search.trim().toLowerCase();
    const base = q
      ? this.list.filter((s) => s.name.toLowerCase().includes(q) || s.host.toLowerCase().includes(q) || s.username.toLowerCase().includes(q))
      : this.list;
    return [...base].sort((a, b) => a.displayName.localeCompare(b.displayName, undefined, { sensitivity: 'base' }));
  });

  groups = $derived.by((): GroupRow[] => {
    const rows: GroupRow[] = [];
    const recents = this.filtered.filter((s) => s.lastConnectedAt).sort((a, b) => (b.lastConnectedAt! > a.lastConnectedAt! ? 1 : -1)).slice(0, 5);
    if (recents.length > 0 && !this.search.trim()) rows.push({ key: RECENT_GROUP, title: t('Str_Recent'), isRecent: true, sessions: recents });
    const map = new Map<string, SessionView[]>();
    for (const s of this.filtered) {
      const g = s.effectiveGroup;
      if (!map.has(g)) map.set(g, []);
      map.get(g)!.push(s);
    }
    const keys = [...map.keys()].sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }));
    for (const k of keys) rows.push({ key: k, title: k, isRecent: false, sessions: map.get(k)! });
    return rows;
  });

  /** Thứ tự hiển thị phẳng (để Shift+nhấp chọn dải). */
  visibleOrder = $derived.by(() => {
    const out: string[] = [];
    for (const g of this.groups) if (!this.collapsed.has(g.key)) for (const s of g.sessions) out.push(`${g.key}\u0000${s.id}`);
    return out;
  });

  get existingGroups() { return [...new Set(this.list.map((s) => s.group).filter((g) => g.trim()))].sort(); }
  byId(id: string) { return this.list.find((s) => s.id === id); }
  get selected(): SessionView[] { return this.selectedIds.map((id) => this.byId(id)).filter((s): s is SessionView => !!s); }

  select(id: string, mode: 'single' | 'toggle' | 'range' = 'single', groupKey?: string) {
    if (mode === 'single') { this.selectedIds = [id]; this.anchorId = id; return; }
    if (mode === 'toggle') {
      this.selectedIds = this.selectedIds.includes(id) ? this.selectedIds.filter((x) => x !== id) : [...this.selectedIds, id];
      this.anchorId = id; return;
    }
    const order = this.visibleOrder.map((k) => k.split('\u0000')[1]);
    const a = order.indexOf(this.anchorId ?? id);
    const b = order.indexOf(id);
    if (a < 0 || b < 0) { this.selectedIds = [id]; this.anchorId = id; return; }
    const [lo, hi] = a < b ? [a, b] : [b, a];
    this.selectedIds = [...new Set(order.slice(lo, hi + 1))];
    void groupKey;
  }
  selectAll() { this.selectedIds = this.filtered.map((s) => s.id); }
  isSelected(id: string) { return this.selectedIds.includes(id); }

  async toggleGroup(key: string) {
    const next = new Set(this.collapsed);
    if (next.has(key)) next.delete(key); else next.add(key);
    this.collapsed = next;
    await invoke('save_collapsed_groups', { groups: [...next].filter((k) => k !== RECENT_GROUP) });
  }

  async save(draft: SessionDraft): Promise<SessionView> {
    const s = await invoke<SessionView>('save_session', { draft });
    await this.load();
    return s;
  }
  async remove(ids: string[]) { await invoke('delete_sessions', { ids }); this.selectedIds = []; await this.load(); }
  async duplicate(id: string) { await invoke('duplicate_session', { id }); await this.load(); }
  async moveToGroup(ids: string[], group: string) { await invoke('move_sessions_to_group', { ids, group }); await this.load(); }
  async renameGroup(oldName: string, newName: string) { await invoke('rename_group', { oldName, newName }); await this.load(); }
  setLive(id: string, status: 'connected' | 'connecting' | null) {
    const next = { ...this.live };
    if (status) next[id] = status; else delete next[id];
    this.live = next;
  }
}
export const sessions = new SessionsStore();
