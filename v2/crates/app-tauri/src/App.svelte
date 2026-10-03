<script lang="ts">
  // Cửa sổ chính — bố cục y hệt v1 (MainWindow.xaml): toolbar / cột trái (Sessions | SFTP) / splitter / tab bar + terminal / status bar.
  import { onMount } from 'svelte';
  import { initIpc, invoke, isTauri, listen } from './lib/ipc';
  import { t } from './lib/i18n.svelte';
  import type { DialogRequest, MonitorInfo, SessionView, TabStatusEvent } from './lib/types';
  import { settings } from './stores/settings.svelte';
  import { sessions } from './stores/sessions.svelte';
  import { tabs, type Tab } from './stores/tabs.svelte';
  import { ui } from './stores/ui.svelte';
  import Toolbar from './components/Toolbar.svelte';
  import StatusBar from './components/StatusBar.svelte';
  import SessionList from './components/sessions/SessionList.svelte';
  import TabBar from './components/terminal/TabBar.svelte';
  import TerminalPane from './components/terminal/TerminalPane.svelte';
  import SftpPanel from './components/sftp/SftpPanel.svelte';
  import MessageBoxes from './components/MessageBoxes.svelte';
  import ContextMenu from './components/ContextMenu.svelte';
  import SessionEditor from './components/dialogs/SessionEditor.svelte';
  import SettingsDialog from './components/dialogs/SettingsDialog.svelte';
  import ExportDialog from './components/dialogs/ExportDialog.svelte';
  import ImportDialog from './components/dialogs/ImportDialog.svelte';
  import HostKeyDialog from './components/dialogs/HostKeyDialog.svelte';
  import PasswordDialog from './components/dialogs/PasswordDialog.svelte';

  let ready = $state(false);
  let leftWidth = $state(400);
  let editor = $state<{ editing: SessionView | null; group: string } | null>(null);
  let showSettings = $state(false);
  let exportSel = $state<SessionView[] | null | undefined>(undefined);
  let importPath = $state<string | null | undefined>(undefined);
  let dialogReq = $state<DialogRequest | null>(null);
  let sessionListRef: ReturnType<typeof SessionList> | undefined = $state();
  let sftpRef: ReturnType<typeof SftpPanel> | undefined = $state();
  const panes = new Map<string, ReturnType<typeof TerminalPane>>();
  let paneRefs = $state<Record<string, ReturnType<typeof TerminalPane>>>({});

  const statusText = $derived.by(() => {
    const tb = tabs.selected;
    if (!tb) return t('Str_Ready');
    return t('Str_ViewingTab', tb.title, tb.session.subtitle);
  });
  $effect(() => { const tb = tabs.selected; document.title = tb ? `${tb.title} — SN Term` : 'SN Term'; });

  // ---- Hành động ----
  function connect(s: SessionView) { tabs.open(s); }
  async function connectMany(list: SessionView[]) {
    if (list.length >= 10 && !(await ui.confirm(t('Str_ConfirmOpenManyTitle'), t('Str_ConfirmOpenMany', list.length)))) return;
    tabs.openMany(list);
  }
  function connectSelected() {
    const sel = sessions.selected;
    if (sel.length > 1) connectMany(sel); else if (sel[0]) connect(sel[0]); else if (sessions.list[0]) connect(sessions.list[0]); else editor = { editing: null, group: '' };
  }
  async function deleteSessions(list: SessionView[]) {
    const msg = list.length === 1 ? t('Str_ConfirmDeletePrompt', list[0].displayName) : t('Str_ConfirmDeleteMultiplePrompt', list.length) + '\n' + list.slice(0, 5).map((s) => `- ${s.displayName}`).join('\n');
    if (await ui.confirm(t('Str_ConfirmDeleteTitle'), msg, 'warn')) await sessions.remove(list.map((s) => s.id));
  }
  async function closeTab(tab: Tab) {
    if (tab.status === 'connected' && !(await ui.confirm(t('Str_ConfirmCloseTitle'), t('Str_ConfirmCloseTab'), 'question'))) return;
    await tabs.close(tab.localId);
  }
  function hotkey(name: string) {
    const cur = tabs.selected;
    switch (name) {
      case 'CloseTab': if (cur) closeTab(cur); break;
      case 'NextTab': tabs.selectNext(); break;
      case 'PrevTab': tabs.selectPrev(); break;
      case 'ZoomIn': cur && paneRefs[cur.localId]?.zoom(1); break;
      case 'ZoomOut': cur && paneRefs[cur.localId]?.zoom(-1); break;
      case 'ZoomReset': cur && paneRefs[cur.localId]?.zoom(0); break;
      default: if (name.startsWith('Tab')) tabs.selectIndex(parseInt(name.slice(3), 10) - 1);
    }
  }
  function onWindowKey(e: KeyboardEvent) {
    if (ui.boxes.length || editor || showSettings || exportSel !== undefined || importPath !== undefined || dialogReq) return;
    if (e.ctrlKey && e.key === 'Tab') { e.preventDefault(); e.shiftKey ? tabs.selectPrev() : tabs.selectNext(); }
    else if (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 'w') { e.preventDefault(); if (tabs.selected) closeTab(tabs.selected); }
    else if (e.altKey && e.key >= '1' && e.key <= '9') { e.preventDefault(); tabs.selectIndex(parseInt(e.key, 10) - 1); }
  }
  function answerDialog(answer: Record<string, unknown>) {
    const r = dialogReq; dialogReq = null;
    if (r) invoke('dialog_answer', { requestId: r.requestId, answer }).catch(() => {});
  }

  // ---- Splitter ----
  function startDrag(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX, startW = leftWidth;
    const move = (ev: MouseEvent) => { leftWidth = Math.max(140, Math.min(600, startW + ev.clientX - startX)); };
    const up = () => { window.removeEventListener('mousemove', move); window.removeEventListener('mouseup', up); saveWindowState(); };
    window.addEventListener('mousemove', move); window.addEventListener('mouseup', up);
  }
  function saveWindowState() { invoke('save_window_state', { width: window.outerWidth, height: window.outerHeight, leftColumnWidth: leftWidth }).catch(() => {}); }

  onMount(async () => {
    await initIpc();
    await settings.load();
    leftWidth = settings.value.LeftColumnWidth >= 140 && settings.value.LeftColumnWidth <= 600 ? settings.value.LeftColumnWidth : 400;
    await sessions.init();
    if (sessions.recovered) ui.message(t('Str_WarningTitle'), t('Str_RecoveredCorrupt'), 'warn');
    ready = true;

    await listen('tab:status', (p) => { const e = p as TabStatusEvent; tabs.setStatus(e.tabId, e.status); });
    await listen('tab:monitor', (p) => { const e = p as { tabId: string; info: MonitorInfo | null }; tabs.setMonitor(e.tabId, e.info); });
    await listen('tab:connected', (p) => { const tb = tabs.byTabId(p as string); if (tb && tabs.selected === tb) tabs.leftTab = 1; });
    await listen('dialog:request', (p) => { dialogReq = p as DialogRequest; });
    await listen('app:open-file', (p) => { importPath = p as string; });

    if (isTauri) {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const win = getCurrentWindow();
      await win.onDragDropEvent((ev) => {
        if (ev.payload.type !== 'drop') return;
        const paths = ev.payload.paths as string[];
        const importFile = paths.find((f) => /\.(snterm|mxtsessions|ini)$/i.test(f));
        const target = document.elementFromPoint(ev.payload.position.x, ev.payload.position.y);
        if (target?.closest('.sftp-host') && tabs.leftTab === 1 && tabs.selected?.status === 'connected') { sftpRef?.uploadPaths(paths); }
        else if (importFile) importPath = importFile;
      });
      await win.onCloseRequested(async (ev) => {
        const n = tabs.connectedCount;
        if (n > 0) { ev.preventDefault(); if (!(await ui.confirm(t('Str_ConfirmCloseTitle'), t('Str_ConfirmCloseApp', n), 'question'))) return; }
        saveWindowState();
        await tabs.closeAll();
        if (n > 0) win.destroy();
      });
      window.addEventListener('resize', () => { clearTimeout(resizeSave); resizeSave = setTimeout(saveWindowState, 800); });
    }
  });
  let resizeSave: ReturnType<typeof setTimeout>;
</script>

<svelte:window onkeydown={onWindowKey} />

{#if ready}
<div class="root">
  <Toolbar onconnect={connectSelected} onadd={() => (editor = { editing: null, group: '' })} onexport={() => (exportSel = null)} onimport={() => (importPath = null)} onsettings={() => (showSettings = true)} />
  <div class="main">
    <div class="left" style="width:{leftWidth}px">
      <div class="left-tabs">
        <button class="ltab" class:active={tabs.leftTab === 0} onclick={() => (tabs.leftTab = 0)}>{t('Str_Sessions')}</button>
        <button class="ltab" class:active={tabs.leftTab === 1} onclick={() => (tabs.leftTab = 1)}>{t('Str_Sftp')}</button>
      </div>
      <div class="left-body" style:display={tabs.leftTab === 0 ? 'block' : 'none'}>
        <SessionList bind:this={sessionListRef} onconnect={connect} onconnectmany={connectMany} onedit={(s) => (editor = { editing: s, group: '' })} onexport={(l) => (exportSel = l)} ondelete={deleteSessions} />
      </div>
      <div class="left-body sftp-host" style:display={tabs.leftTab === 1 ? 'block' : 'none'}>
        <SftpPanel bind:this={sftpRef} tab={tabs.selected} />
      </div>
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="splitter" onmousedown={startDrag}></div>
    <div class="right">
      <TabBar onopenlist={() => { tabs.leftTab = 0; sessionListRef?.focusSearch(); }} onclose={closeTab} />
      <div class="term-host">
        {#each tabs.list as tab (tab.localId)}
          <TerminalPane bind:this={paneRefs[tab.localId]} {tab} visible={tabs.selectedLocalId === tab.localId} onhotkey={hotkey} />
        {/each}
        {#if tabs.list.length === 0}
          <div class="placeholder muted">{t('Str_TerminalPlaceholder')}</div>
        {/if}
      </div>
    </div>
  </div>
  <StatusBar text={statusText} />
</div>
{/if}

{#if editor}
  <SessionEditor editing={editor.editing} initialGroup={editor.group} oncancel={() => (editor = null)} onsaved={(s, c) => { editor = null; if (c) connect(s); }} />
{/if}
{#if showSettings}<SettingsDialog onclose={() => (showSettings = false)} />{/if}
{#if exportSel !== undefined}<ExportDialog preselected={exportSel} onclose={() => (exportSel = undefined)} />{/if}
{#if importPath !== undefined}<ImportDialog initialPath={importPath} onclose={() => (importPath = undefined)} onimported={() => sessions.load()} />{/if}
{#if dialogReq?.kind === 'hostKey'}
  <HostKeyDialog req={dialogReq} onanswer={(d) => answerDialog({ kind: 'hostKey', decision: d })} />
{:else if dialogReq?.kind === 'password'}
  <PasswordDialog req={dialogReq} onanswer={(pw, save) => answerDialog({ kind: 'password', password: pw, save })} />
{/if}
<MessageBoxes />
<ContextMenu />

<style>
  .root { display: flex; flex-direction: column; height: 100%; }
  .main { flex: 1; display: flex; min-height: 0; }
  .left { display: flex; flex-direction: column; background: var(--theme-sidebar-bg); border-right: 1px solid var(--theme-border); min-width: 140px; max-width: 600px; }
  .left-tabs { display: flex; border-bottom: 1px solid var(--theme-border); }
  .ltab { flex: 0 0 auto; padding: 6px 14px; border: none; background: transparent; color: var(--theme-fg-muted); font-weight: 600; cursor: pointer; border-bottom: 2px solid transparent; }
  .ltab.active { color: var(--theme-fg); border-bottom-color: var(--theme-connect-bg); }
  .ltab:hover { color: var(--theme-fg); }
  .left-body { flex: 1; min-height: 0; overflow: hidden; height: 100%; }
  .splitter { width: 5px; cursor: col-resize; background: var(--theme-border); flex: none; }
  .splitter:hover { background: var(--theme-button-hover-border); }
  .right { flex: 1; display: flex; flex-direction: column; min-width: 0; }
  .term-host { flex: 1; position: relative; background: var(--terminal-bg); min-height: 0; }
  .placeholder { position: absolute; inset: 0; display: flex; align-items: center; justify-content: center; font-style: italic; padding: 20px; text-align: center; }
</style>
