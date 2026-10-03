<script lang="ts">
  // Trình duyệt SFTP cho tab đang chọn (giống SftpPanel.xaml v1): toolbar, ô đường dẫn, bảng file, thanh tiến trình.
  import { invoke, listen } from '../../lib/ipc';
  import { t } from '../../lib/i18n.svelte';
  import { formatEpoch, formatSize, formatSpeed, TypeAhead } from '../../lib/format';
  import type { SftpItem, SftpListing, TransferProgress } from '../../lib/types';
  import { settings } from '../../stores/settings.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { tabs, type Tab } from '../../stores/tabs.svelte';
  import ChmodDialog from '../dialogs/ChmodDialog.svelte';
  import { onMount, untrack } from 'svelte';

  let { tab }: { tab: Tab | null } = $props();

  // Trạng thái riêng từng tab (giữ đường dẫn khi chuyển qua lại).
  const perTab = new Map<string, { path: string; home: string; items: SftpItem[] }>();
  let items = $state<SftpItem[]>([]);
  let path = $state('');
  let pathInput = $state('');
  let loading = $state(false);
  let status = $state('');
  let selected = $state<Set<string>>(new Set());
  let anchor = $state<string | null>(null);
  let transfer = $state<TransferProgress | null>(null);
  let chmodOpen = $state(false);
  let listEl: HTMLDivElement | undefined = $state();
  let loadedForTab = $state<string | null>(null);
  const typeAhead = new TypeAhead();

  let showHidden = $derived(settings.value.ShowHiddenFiles);
  let connected = $derived(!!tab?.id && tab.status === 'connected');

  $effect(() => {
    // Khi đổi tab hoặc tab vừa kết nối → nạp thư mục (home) của VM đó.
    const id = tab?.id; const st = tab?.status;
    untrack(() => {
      if (!id || st !== 'connected') { if (!tab) { items = []; path = ''; pathInput = ''; loadedForTab = null; } return; }
      if (loadedForTab === id) return;
      loadedForTab = id;
      const cached = perTab.get(id);
      if (cached) { items = cached.items; path = cached.path; pathInput = cached.path; status = t('Str_ItemsCount', cached.items.length - 1); }
      else load(undefined);
    });
  });

  async function load(p: string | undefined) {
    const id = tab?.id; if (!id) return;
    loading = true; status = p === undefined ? t('Str_ConnectingSftp') : t('Str_LoadingFiles');
    try {
      const r = await invoke<SftpListing>('sftp_list', { tabId: id, path: p ?? null, showHidden });
      if (tab?.id !== id) return;
      items = r.items; path = r.path; pathInput = r.path; selected = new Set(); anchor = null;
      perTab.set(id, { path: r.path, home: r.home, items: r.items });
      status = t('Str_ItemsCount', r.items.length - 1);
    } catch (e) {
      status = String(e);
      if (String(e).toLowerCase().includes('quyền') || String(e).toLowerCase().includes('permission')) ui.message(t('Str_PermissionDeniedTitle'), t('Str_PermissionDenied'), 'warn');
    } finally { loading = false; }
  }
  const refresh = () => load(path || undefined);
  const goUp = () => { const parent = items.find((i) => i.isParentDirectory)?.fullName ?? '/'; load(path === '/' ? '/' : parent); };
  const goHome = () => load(perTab.get(tab?.id ?? '')?.home || undefined);
  let lastHidden = showHidden;
  $effect(() => {
    const h = showHidden;
    untrack(() => { if (h !== lastHidden) { lastHidden = h; if (tab?.id && loadedForTab === tab.id && path) load(path); } });
  });

  function selItems(): SftpItem[] { return items.filter((i) => selected.has(i.fullName) && !i.isParentDirectory); }
  function clickRow(e: MouseEvent, it: SftpItem) {
    if (e.button === 2) { if (!selected.has(it.fullName)) { selected = new Set([it.fullName]); anchor = it.fullName; } return; }
    if (e.button !== 0) return;
    if (e.shiftKey && anchor) {
      const a = items.findIndex((i) => i.fullName === anchor), b = items.findIndex((i) => i.fullName === it.fullName);
      const [lo, hi] = a < b ? [a, b] : [b, a];
      selected = new Set(items.slice(lo, hi + 1).map((i) => i.fullName));
    } else if (e.ctrlKey) { const n = new Set(selected); if (n.has(it.fullName)) n.delete(it.fullName); else n.add(it.fullName); selected = n; anchor = it.fullName; }
    else { selected = new Set([it.fullName]); anchor = it.fullName; }
    listEl?.focus();
  }
  function open(it: SftpItem) {
    if (it.isParentDirectory) goUp();
    else if (it.isDirectory) load(it.fullName);
    else editDefault(it);
  }
  async function onKey(e: KeyboardEvent) {
    const sel = selItems();
    if (e.key === 'Enter') { e.preventDefault(); const one = items.find((i) => selected.has(i.fullName)); if (one) open(one); return; }
    if (e.key === 'Backspace') { e.preventDefault(); goUp(); return; }
    if (e.key === 'Delete') { e.preventDefault(); if (sel.length) del(sel); return; }
    if (e.key === 'F2') { e.preventDefault(); if (sel[0]) rename(sel[0]); return; }
    if (e.key === 'F5') { e.preventDefault(); refresh(); return; }
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const cur = items.findIndex((i) => selected.has(i.fullName));
      const next = e.key === 'ArrowDown' ? Math.min(items.length - 1, cur + 1) : Math.max(0, cur - 1);
      selected = new Set([items[next].fullName]); anchor = items[next].fullName; scrollTo(next); return;
    }
    if (e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
      const cur = items.findIndex((i) => selected.has(i.fullName));
      const idx = typeAhead.next(e.key, items.map((i) => i.name), cur);
      if (idx >= 0) { selected = new Set([items[idx].fullName]); anchor = items[idx].fullName; scrollTo(idx); }
      e.preventDefault();
    }
  }
  function scrollTo(idx: number) { listEl?.querySelectorAll('.row')[idx]?.scrollIntoView({ block: 'nearest' }); }

  async function mkdir() {
    const name = await ui.input(t('Str_NewFolderTitle'), t('Str_NewFolderPrompt'));
    if (!name?.trim()) return;
    try { await invoke('sftp_mkdir', { tabId: tab!.id, path: `${path.replace(/\/+$/, '')}/${name.trim()}` }); await refresh(); }
    catch (e) { ui.message(t('Str_Error'), t('Str_ErrorCreateDir', String(e)), 'error'); }
  }
  async function rename(it: SftpItem) {
    const name = await ui.input(t('Str_RenameTitle'), t('Str_RenamePrompt'), it.name);
    if (!name?.trim() || name.trim() === it.name) return;
    const parent = it.fullName.substring(0, it.fullName.lastIndexOf('/')) || '/';
    try { await invoke('sftp_rename', { tabId: tab!.id, from: it.fullName, to: `${parent.replace(/\/+$/, '')}/${name.trim()}` }); await refresh(); }
    catch (e) { ui.message(t('Str_Error'), t('Str_ErrorRename', String(e)), 'error'); }
  }
  async function del(list: SftpItem[]) {
    const msg = list.length === 1 ? t('Str_ConfirmDeletePrompt', list[0].name) : t('Str_ConfirmDeleteMultiplePrompt', list.length) + '\n' + list.slice(0, 5).map((i) => `- ${i.name}`).join('\n');
    if (!(await ui.confirm(t('Str_ConfirmDeleteTitle'), msg, 'warn'))) return;
    loading = true;
    try { await invoke('sftp_remove', { tabId: tab!.id, paths: list.map((i) => [i.fullName, i.isDirectory]) }); await refresh(); }
    catch (e) { ui.message(t('Str_Error'), t('Str_ErrorDelete', String(e)), 'error'); }
    finally { loading = false; }
  }
  async function chmodApply(mode: number, recursive: boolean) {
    chmodOpen = false; const list = selItems(); if (!list.length) return;
    status = t('Str_ChangingPermissions'); loading = true;
    try { await invoke('sftp_chmod', { tabId: tab!.id, paths: list.map((i) => [i.fullName, i.isDirectory]), mode, recursive }); status = t('Str_PermissionsSuccess'); await refresh(); }
    catch (e) { status = t('Str_CannotChangePerms', String(e)); ui.message(t('Str_PermissionDeniedTitle'), t('Str_CannotChangePerms', String(e)), 'warn'); }
    finally { loading = false; }
  }
  async function editDefault(it: SftpItem) {
    if (it.isDirectory || it.isParentDirectory) return;
    await editWith(it, settings.value.CustomEditorPath || null);
  }
  async function editWith(it: SftpItem, editor: string | null) {
    status = t('Str_Downloading', it.name); loading = true;
    try { await invoke('sftp_open_in_editor', { tabId: tab!.id, remotePath: it.fullName, editor }); status = t('Str_OpeningFileStatus', it.name); }
    catch (e) { ui.message(t('Str_Error'), t('Str_ErrorOpenFile', String(e)), 'error'); }
    finally { loading = false; }
  }
  async function editWithApp(it: SftpItem) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const f = await open({ title: t('Str_ChooseEditorApp'), multiple: false, filters: [{ name: 'Executable', extensions: ['exe'] }, { name: t('Str_AllFiles'), extensions: ['*'] }] }).catch(() => null);
    if (typeof f === 'string') await editWith(it, f);
  }
  async function upload() {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const f = await open({ title: t('Str_ChooseUploadFiles'), multiple: true }).catch(() => null);
    const files = Array.isArray(f) ? f : f ? [f] : [];
    if (files.length) await uploadPaths(files as string[]);
  }
  export async function uploadPaths(paths: string[]) {
    if (!tab?.id || !path) return;
    const errors = await invoke<string[]>('sftp_upload', { tabId: tab.id, localPaths: paths, remoteDir: path }).catch((e) => [String(e)]);
    for (const err of errors) ui.message(t('Str_Error'), err, 'error');
    await refresh();
  }
  async function download() {
    const list = selItems(); if (!list.length) return;
    const { open } = await import('@tauri-apps/plugin-dialog');
    const dir = await open({ title: t('Str_ChooseDownloadFolder'), directory: true }).catch(() => null);
    if (typeof dir !== 'string') return;
    const errors = await invoke<string[]>('sftp_download', { tabId: tab!.id, items: list.map((i) => [i.fullName, i.isDirectory, i.size]), destDir: dir }).catch((e) => [String(e)]);
    for (const err of errors) ui.message(t('Str_Error'), err, 'error');
  }
  function cancelTransfer() { if (tab?.id) invoke('sftp_cancel_transfer', { tabId: tab.id }); transfer = null; }
  function rowContext(e: MouseEvent, it: SftpItem) {
    if (it.isParentDirectory) { e.preventDefault(); return; }
    if (!selected.has(it.fullName)) { selected = new Set([it.fullName]); anchor = it.fullName; }
    const sel = selItems();
    ui.openMenu(e, [
      { label: t('Str_EditFileDefault'), disabled: it.isDirectory, action: () => editDefault(it) },
      { label: t('Str_EditFileWith'), disabled: it.isDirectory, action: () => editWithApp(it) },
      { label: t('Str_Download'), action: download },
      { sep: true, label: '' },
      { label: t('Str_Rename'), shortcut: 'F2', disabled: sel.length > 1, action: () => rename(it) },
      { label: t('Str_Chmod'), action: () => (chmodOpen = true) },
      { label: t('Str_Delete'), shortcut: 'Delete', danger: true, action: () => del(sel) },
      { sep: true, label: '' },
      { label: t('Str_NewDirectory'), action: mkdir },
      { label: t('Str_Refresh'), shortcut: 'F5', action: refresh },
      { label: t('Str_CopyPath'), action: () => invoke('clipboard_write', { text: it.fullName }) },
    ]);
  }
  function emptyContext(e: MouseEvent) {
    if ((e.target as HTMLElement).closest('.row')) return;
    ui.openMenu(e, [{ label: t('Str_NewDirectory'), action: mkdir }, { label: t('Str_Refresh'), shortcut: 'F5', action: refresh }, { label: t('Str_Upload'), action: upload }]);
  }

  onMount(() => {
    const unsubs: (() => void)[] = [];
    listen('transfer:progress', (p) => { const tp = p as TransferProgress; if (tp.tabId !== tab?.id) return; transfer = tp.finished ? null : tp; if (tp.finished && tp.error) ui.message(t('Str_Error'), tp.error, 'error'); }).then((u) => unsubs.push(u));
    listen('sftp:autosaved', (p) => { const a = p as { tabId: string; name: string; ok: boolean }; if (a.tabId === tab?.id && a.ok) { status = t('Str_AutoSavedStatus', a.name); refresh(); } }).then((u) => unsubs.push(u));
    return () => unsubs.forEach((u) => u());
  });
  const chmodTarget = $derived(selItems().length === 1 ? selItems()[0].name : t('Str_ItemsCount', selItems().length));
</script>

{#if !tab || !connected}
  <div class="none muted">{t('Str_NoVmConnected')}</div>
{:else}
  <div class="panel">
    <div class="tools">
      <button class="btn small" onclick={goUp} title={t('Str_UpFolderTooltip')}>{t('Str_UpFolder')}</button>
      <button class="btn small" onclick={refresh} title={t('Str_RefreshTooltip')}>⟳</button>
      <button class="btn small" onclick={goHome} title={t('Str_HomeTooltip')}>🏠</button>
      <button class="btn small" onclick={mkdir}>{t('Str_NewFolderBtn')}</button>
      <button class="btn small" onclick={upload}>{t('Str_Upload')}</button>
      <button class="btn small" onclick={download} disabled={selItems().length === 0}>{t('Str_Download')}</button>
      <button class="btn small" onclick={() => { const s = selItems()[0]; if (s) editDefault(s); }} disabled={selItems().length !== 1 || selItems()[0].isDirectory} title={t('Str_EditDefaultTooltip')}>{t('Str_EditBtn')}</button>
      <label class="check small"><input type="checkbox" checked={showHidden} onchange={(e) => settings.save({ ...settings.value, ShowHiddenFiles: (e.currentTarget as HTMLInputElement).checked })} />{t('Str_ShowHiddenFilesShort')}</label>
    </div>
    <input class="input path" bind:value={pathInput} onkeydown={(e) => { if (e.key === 'Enter') load(pathInput.trim()); }} spellcheck="false" />
    <div class="head">
      <span class="c name">{t('Str_ColName')}</span><span class="c size">{t('Str_ColSize')}</span><span class="c perm">{t('Str_ColPermissions')}</span>
      <span class="c own">{t('Str_ColOwner')}</span><span class="c grp">{t('Str_ColGroup')}</span><span class="c mod">{t('Str_ColModified')}</span>
    </div>
    <!-- svelte-ignore a11y_no_noninteractive_tabindex a11y_no_static_element_interactions -->
    <div class="list" bind:this={listEl} tabindex="0" onkeydown={onKey} oncontextmenu={emptyContext} role="listbox">
      {#each items as it (it.fullName + '|' + it.name)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div class="row" class:selected={selected.has(it.fullName)} role="option" aria-selected={selected.has(it.fullName)}
             onmousedown={(e) => clickRow(e, it)} ondblclick={() => open(it)} oncontextmenu={(e) => rowContext(e, it)} title={it.fullName}>
          <span class="c name"><span class="ico">{it.isParentDirectory ? '⤴' : it.isSymbolicLink ? (it.isDirectory ? '📁🔗' : '📄🔗') : it.isDirectory ? '📁' : '📄'}</span>{it.name}</span>
          <span class="c size">{formatSize(it.size, it.isDirectory, it.isParentDirectory)}</span>
          <span class="c perm">{it.permissions}</span>
          <span class="c own">{it.owner}</span>
          <span class="c grp">{it.group}</span>
          <span class="c mod">{formatEpoch(it.lastModified, it.isParentDirectory)}</span>
        </div>
      {/each}
      {#if loading}<div class="loading muted">{t('Str_Loading')}</div>{/if}
    </div>
    {#if transfer}
      <div class="transfer">
        <div class="tline"><span class="tname">{transfer.name}</span><span class="muted"> ({formatSpeed(transfer.speedBps)})</span><button class="btn small" onclick={cancelTransfer}>{t('Str_Cancel')}</button></div>
        <div class="bar"><div class="fill" style="width:{transfer.total ? Math.min(100, (transfer.done * 100) / transfer.total) : 0}%"></div></div>
      </div>
    {/if}
    <div class="status muted" title={status}>{status}</div>
  </div>
  {#if chmodOpen}
    <ChmodDialog target={chmodTarget} currentPerms={selItems()[0]?.permissions ?? ''} hasDirectory={selItems().some((i) => i.isDirectory)} onok={chmodApply} oncancel={() => (chmodOpen = false)} />
  {/if}
{/if}

<style>
  .none { display: flex; align-items: center; justify-content: center; height: 100%; }
  .panel { display: flex; flex-direction: column; height: 100%; padding: 6px; gap: 6px; }
  .tools { display: flex; flex-wrap: wrap; gap: 4px; align-items: center; }
  .check.small { font-size: 12px; margin-left: 4px; }
  .path { padding: 4px 6px; font-family: 'JetBrains Mono', Consolas, monospace; font-size: 12px; }
  .head, .row { display: grid; grid-template-columns: minmax(130px, 1fr) 65px 95px 65px 65px 105px; gap: 6px; align-items: center; }
  .head { font-size: 11px; color: var(--theme-fg-muted); padding: 2px 6px; border-bottom: 1px solid var(--theme-border); }
  .list { flex: 1; overflow: auto; outline: none; border: 1px solid var(--theme-border); border-radius: 3px; background: var(--theme-input-bg); min-height: 80px; }
  .row { padding: 3px 6px; font-size: 12px; cursor: default; white-space: nowrap; }
  .row .c { overflow: hidden; text-overflow: ellipsis; }
  .row:hover { background: var(--theme-item-hover); color: var(--theme-item-selected-fg); }
  .row.selected { background: var(--theme-item-selected); color: var(--theme-item-selected-fg); }
  .ico { margin-right: 6px; }
  .loading { padding: 10px; text-align: center; font-style: italic; }
  .transfer { padding: 4px 2px; }
  .tline { display: flex; align-items: center; gap: 4px; font-size: 12px; }
  .tname { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 220px; }
  .tline .btn { margin-left: auto; }
  .bar { height: 6px; background: var(--theme-border); border-radius: 3px; margin: 4px 0 0; overflow: hidden; }
  .fill { height: 100%; background: var(--theme-connect-bg); transition: width 0.2s; }
  .status { font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>
