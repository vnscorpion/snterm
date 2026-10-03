<script lang="ts">
  import { sessions, RECENT_GROUP } from '../../stores/sessions.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { t } from '../../lib/i18n.svelte';
  import type { SessionView } from '../../lib/types';

  let { onconnect, onconnectmany, onedit, onexport, ondelete }: {
    onconnect: (s: SessionView) => void; onconnectmany: (l: SessionView[]) => void; onedit: (s: SessionView) => void;
    onexport: (l: SessionView[]) => void; ondelete: (l: SessionView[]) => void;
  } = $props();

  let listEl: HTMLDivElement | undefined = $state();
  let searchEl: HTMLInputElement | undefined = $state();
  export function focusSearch() { searchEl?.focus(); }

  function rowKey(groupKey: string, id: string) { return `${groupKey}\u0000${id}`; }

  function onRowMouseDown(e: MouseEvent, s: SessionView, groupKey: string) {
    if (e.button === 2) { if (!sessions.isSelected(s.id)) sessions.select(s.id, 'single', groupKey); return; }
    if (e.button !== 0) return;
    if (e.shiftKey) sessions.select(s.id, 'range', groupKey);
    else if (e.ctrlKey || e.metaKey) sessions.select(s.id, 'toggle', groupKey);
    else sessions.select(s.id, 'single', groupKey);
    listEl?.focus();
  }
  function onRowContext(e: MouseEvent, s: SessionView) {
    if (!sessions.isSelected(s.id)) sessions.select(s.id, 'single');
    const sel = sessions.selected;
    const many = sel.length > 1;
    ui.openMenu(e, [
      { label: many ? t('Str_ConnectSelected', sel.length) : t('Str_ConnectMenu'), action: () => (many ? onconnectmany(sel) : onconnect(s)) },
      { label: t('Str_EditVm'), shortcut: 'F2', disabled: many, action: () => onedit(s) },
      { label: t('Str_Duplicate'), shortcut: 'Ctrl+D', disabled: many, action: () => sessions.duplicate(s.id) },
      { label: t('Str_MoveToGroup'), action: moveToGroup },
      { label: t('Str_Ungroup'), action: () => sessions.moveToGroup(sel.map((x) => x.id), '') },
      { sep: true, label: '' },
      { label: many ? t('Str_ExportSelected', sel.length) : t('Str_ExportVm'), action: () => onexport(sel) },
      { sep: true, label: '' },
      { label: t('Str_Delete'), shortcut: 'Delete', danger: true, action: () => ondelete(sel) },
    ]);
  }
  async function moveToGroup() {
    const sel = sessions.selected; if (sel.length === 0) return;
    const v = await ui.input(t('Str_MoveToGroupTitle'), t('Str_EnterGroupName'), sel[0].group);
    if (v !== null) await sessions.moveToGroup(sel.map((x) => x.id), v);
  }
  function onGroupContext(e: MouseEvent, key: string, title: string, list: SessionView[]) {
    if (key === RECENT_GROUP) return;
    ui.openMenu(e, [
      { label: t('Str_ConnectAllInGroup'), action: () => onconnectmany(list) },
      { label: t('Str_RenameGroup'), action: async () => { const v = await ui.input(t('Str_RenameGroupTitle'), t('Str_EnterNewGroupName'), title); if (v && v.trim()) await sessions.renameGroup(title, v); } },
      { label: t('Str_ExportGroup'), action: () => onexport(list) },
      { sep: true, label: '' },
      { label: t('Str_DeleteGroup'), danger: true, action: () => ondelete(list) },
    ]);
  }
  function onKey(e: KeyboardEvent) {
    const sel = sessions.selected;
    if (e.key === 'Enter') { e.preventDefault(); if (sel.length > 1) onconnectmany(sel); else if (sel[0]) onconnect(sel[0]); }
    else if (e.key === 'F2') { e.preventDefault(); if (sel[0]) onedit(sel[0]); }
    else if (e.key === 'Delete') { e.preventDefault(); if (sel.length) ondelete(sel); }
    else if (e.key.toLowerCase() === 'd' && e.ctrlKey) { e.preventDefault(); if (sel[0]) sessions.duplicate(sel[0].id); }
    else if (e.key.toLowerCase() === 'a' && e.ctrlKey) { e.preventDefault(); sessions.selectAll(); }
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const order = sessions.visibleOrder.map((k) => k.split('\u0000')[1]);
      if (order.length === 0) return;
      const cur = sel.length ? order.indexOf(sel[sel.length - 1].id) : -1;
      const next = e.key === 'ArrowDown' ? Math.min(order.length - 1, cur + 1) : Math.max(0, cur - 1);
      sessions.select(order[next], e.shiftKey ? 'range' : 'single');
      listEl?.querySelector(`[data-id="${order[next]}"]`)?.scrollIntoView({ block: 'nearest' });
    }
  }
  function liveColor(s: SessionView) {
    const st = sessions.live[s.id];
    return st === 'connected' ? 'var(--status-connected)' : st === 'connecting' ? 'var(--status-connecting)' : 'var(--status-off)';
  }
  function lastConnected(s: SessionView) {
    if (!s.lastConnectedAt) return t('Str_Never');
    return new Date(s.lastConnectedAt).toLocaleString();
  }
</script>

<div class="wrap">
  <input class="input search" bind:this={searchEl} bind:value={sessions.search} placeholder={t('Str_Search')} />
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="list" bind:this={listEl} tabindex="0" onkeydown={onKey} role="listbox" aria-multiselectable="true">
    {#if sessions.list.length === 0}
      <div class="empty"><div>{t('Str_NoVmsYet')}</div><div class="muted" style="font-size:12px">{t('Str_ClickAddVm')}</div></div>
    {/if}
    {#each sessions.groups as g (g.key)}
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <div class="group" class:recent={g.isRecent} onclick={() => sessions.toggleGroup(g.key)} oncontextmenu={(e) => onGroupContext(e, g.key, g.title, g.sessions)}>
        <span class="chev">{sessions.collapsed.has(g.key) ? '▸' : '▾'}</span>
        <span class="gtitle">{g.title}</span>
        <span class="count">{g.sessions.length}</span>
      </div>
      {#if !sessions.collapsed.has(g.key)}
        {#each g.sessions as s (rowKey(g.key, s.id))}
          <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
          <div class="row" class:selected={sessions.isSelected(s.id)} data-id={s.id} role="option" aria-selected={sessions.isSelected(s.id)}
               title={`${s.subtitle}\n${t('Str_LastConnected')} ${lastConnected(s)}`}
               onmousedown={(e) => onRowMouseDown(e, s, g.key)} ondblclick={() => onconnect(s)} oncontextmenu={(e) => onRowContext(e, s)}>
            <span class="icon" style="color:{liveColor(s)}">
              <svg width="18" height="18" viewBox="0 0 18 18"><rect x="2" y="3" width="14" height="10" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.6"/><path d="M6 16h6" stroke="currentColor" stroke-width="1.6"/><circle cx="9" cy="8" r="1.6" fill="currentColor"/></svg>
            </span>
            <span class="text">
              <span class="name">{s.displayName}</span>
              <span class="sub">{s.subtitle}</span>
            </span>
            {#if s.keyFileMissing}<span class="warn" title={t('Str_MissingKeyTooltip')}>⚠</span>{/if}
          </div>
        {/each}
      {/if}
    {/each}
  </div>
</div>

<style>
  .wrap { display: flex; flex-direction: column; height: 100%; padding: 6px; }
  .search { margin-bottom: 8px; }
  .list { flex: 1; overflow: auto; outline: none; }
  .empty { padding: 30px 10px; text-align: center; color: var(--theme-fg-muted); }
  .group { display: flex; align-items: center; gap: 6px; padding: 5px 6px; margin-top: 2px; cursor: pointer; font-weight: 600; color: var(--theme-fg); border-radius: 4px; }
  .group:hover { background: var(--theme-item-hover); color: var(--theme-item-selected-fg); }
  .group.recent .gtitle { color: var(--theme-button-hover-border); }
  .group:hover.recent .gtitle { color: var(--theme-item-selected-fg); }
  .chev { width: 12px; font-size: 11px; color: var(--theme-fg-muted); }
  .count { margin-left: auto; font-size: 11px; color: var(--theme-fg-muted); font-weight: normal; }
  .row { display: flex; align-items: center; gap: 6px; padding: 4px 8px 4px 20px; margin: 1px 2px; border-radius: 6px; cursor: default; }
  .row:hover { background: var(--theme-item-hover); color: var(--theme-item-selected-fg); }
  .row.selected { background: var(--theme-item-selected); color: var(--theme-item-selected-fg); }
  .icon { display: inline-flex; width: 18px; height: 18px; }
  .text { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .name { font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .sub { font-size: 11px; color: var(--theme-fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .row:hover .sub, .row.selected .sub { color: var(--theme-item-selected-fg); opacity: 0.8; }
  .warn { color: var(--warning); font-weight: bold; }
</style>
