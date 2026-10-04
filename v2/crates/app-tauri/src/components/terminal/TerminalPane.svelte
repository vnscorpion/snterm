<script lang="ts">
  // Một xterm.js cho một tab; tất cả tab nằm trong cùng trang, ẩn/hiện bằng CSS (không hủy khi chuyển tab).
  import { onDestroy, onMount } from 'svelte';
  import { Terminal } from '@xterm/xterm';
  import { FitAddon } from '@xterm/addon-fit';
  import { Unicode11Addon } from '@xterm/addon-unicode11';
  import { WebglAddon } from '@xterm/addon-webgl';
  import '@xterm/xterm/css/xterm.css';
  import { createOutputChannel, invoke } from '../../lib/ipc';
  import { settings } from '../../stores/settings.svelte';
  import { tabs, type Tab } from '../../stores/tabs.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { t } from '../../lib/i18n.svelte';
  import MonitorBar from './MonitorBar.svelte';

  let { tab, visible, onhotkey }: { tab: Tab; visible: boolean; onhotkey: (name: string) => void } = $props();

  let container: HTMLDivElement;
  let term: Terminal;
  let fit: FitAddon;
  let resizeTimer: ReturnType<typeof setTimeout> | null = null;
  let ro: ResizeObserver | null = null;
  let opened = false;

  const themes = {
    dark: { background: '#1e1e1e', foreground: '#d4d4d4', cursor: '#aeafad', selectionBackground: '#264f78' },
    light: { background: '#ffffff', foreground: '#1e1e1e', cursor: '#1e1e1e', selectionBackground: '#add6ff' },
  };
  function fontFamily() {
    const f = settings.value.FontFamily || 'JetBrains Mono';
    return f.includes(',') ? f : `'${f}', 'JetBrains Mono', Consolas, 'Courier New', monospace`;
  }

  function sendResize() {
    if (!tab.id || !opened) return;
    invoke('terminal_resize', { tabId: tab.id, cols: term.cols, rows: term.rows }).catch(() => {});
  }
  function refit() {
    if (!visible || !opened) return;
    const oc = term.cols, or = term.rows;
    try { fit.fit(); } catch {}
    if (term.cols !== oc || term.rows !== or) sendResize();
  }
  function scheduleFit() { if (resizeTimer) clearTimeout(resizeTimer); resizeTimer = setTimeout(refit, 50); }

  export function focus() { term?.focus(); refit(); }
  export function paste(text: string) { term?.paste(text); }
  export function selectAll() { term?.selectAll(); }
  export function clear() { term?.clear(); }
  export function getSelection() { return term?.getSelection() ?? ''; }
  export function hasSelection() { return term?.hasSelection() ?? false; }

  async function copySelection() {
    const text = term.getSelection();
    if (text) await invoke('clipboard_write', { text }).catch(() => {});
  }
  async function requestPaste() {
    const text = await invoke<string>('clipboard_read').catch(() => '');
    if (!text) return;
    const lines = text.split(/\r\n|\r|\n/).filter((_, i, a) => i < a.length - 1 || a[i] !== '').length;
    if (lines > 1 && settings.value.ConfirmMultilinePaste) {
      const ok = await ui.confirm(t('Str_PasteConfirmTitle'), t('Str_PasteConfirm', lines), 'question', t('Str_Paste'), t('Str_Cancel'));
      if (!ok) return;
    }
    term.paste(text);
    term.focus();
  }
  function showContextMenu(e: MouseEvent) {
    const has = term.hasSelection();
    ui.openMenu(e, [
      { label: t('Str_Copy'), shortcut: 'Ctrl+Shift+C', disabled: !has, action: copySelection },
      { label: t('Str_Paste'), shortcut: 'Ctrl+Shift+V', action: requestPaste },
      { sep: true, label: '' },
      { label: t('Str_SelectAllMenu'), action: () => term.selectAll() },
      { label: t('Str_ClearScreen'), action: () => term.clear() },
    ]);
  }

  onMount(async () => {
    try {
      await Promise.all([document.fonts.load(`${settings.value.FontSize}px 'JetBrains Mono'`), document.fonts.load(`bold ${settings.value.FontSize}px 'JetBrains Mono'`)]);
      await document.fonts.ready;
    } catch {}
    term = new Terminal({
      fontFamily: fontFamily(), fontSize: settings.value.FontSize || 14, lineHeight: 1.0, letterSpacing: 0,
      scrollback: settings.value.Scrollback || 10000, cursorBlink: settings.value.CursorBlink, allowProposedApi: true,
      customGlyphs: true, rescaleOverlappingGlyphs: true, rightClickSelectsWord: false,
      theme: settings.isLight ? themes.light : themes.dark,
    });
    fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = '11';
    term.open(container);
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch { /* fallback canvas/DOM */ }
    try { fit.fit(); } catch {}
    opened = true;

    term.onData((data) => { if (tab.id) invoke('terminal_input', { tabId: tab.id, data }).catch(() => {}); });
    term.attachCustomKeyEventHandler((e) => {
      if (e.type !== 'keydown') return true;
      const k = e.key.toLowerCase();
      if (e.ctrlKey && e.shiftKey && k === 'c') { copySelection(); return false; }
      if (e.ctrlKey && e.shiftKey && k === 'v') { requestPaste(); return false; }
      if (e.ctrlKey && e.shiftKey && k === 'w') { onhotkey('CloseTab'); return false; }
      if (e.ctrlKey && e.key === 'Tab') { onhotkey(e.shiftKey ? 'PrevTab' : 'NextTab'); return false; }
      if (e.altKey && e.key >= '1' && e.key <= '9') { onhotkey('Tab' + e.key); return false; }
      if (e.ctrlKey && (e.key === '=' || e.key === '+')) { onhotkey('ZoomIn'); return false; }
      if (e.ctrlKey && (e.key === '-' || e.key === '_')) { onhotkey('ZoomOut'); return false; }
      if (e.ctrlKey && e.key === '0') { onhotkey('ZoomReset'); return false; }
      return true;
    });
    term.element?.addEventListener('mouseup', (e) => {
      if (e.button !== 0 || !settings.value.CopyOnSelect) return;
      const text = term.getSelection();
      if (text) invoke('clipboard_write', { text }).catch(() => {});
    });
    term.element?.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      const wantPaste = (settings.value.RightClickAction === 'Paste') !== e.shiftKey;
      if (wantPaste) requestPaste(); else showContextMenu(e);
    });

    ro = new ResizeObserver(scheduleFit);
    ro.observe(container);

    // Mở kết nối ở backend khi terminal sẵn sàng.
    const channel = createOutputChannel((bytes) => term.write(bytes));
    try {
      const tabId = await invoke<string>('terminal_open', { sessionId: tab.session.id, cols: term.cols, rows: term.rows, onOutput: channel });
      tabs.setBackendId(tab.localId, tabId);
      if (visible) invoke('terminal_set_visible', { tabId, visible: true }).catch(() => {});
    } catch (err) {
      term.writeln(`\x1b[31m${String(err)}\x1b[0m`);
    }
    if (visible) term.focus();
  });

  $effect(() => {
    // Áp dụng cài đặt ngay cho mọi tab.
    const s = settings.value;
    if (!term) return;
    const ff = fontFamily();
    let changed = false;
    if (term.options.fontSize !== s.FontSize) { term.options.fontSize = s.FontSize; changed = true; }
    if (term.options.fontFamily !== ff) { term.options.fontFamily = ff; changed = true; }
    term.options.theme = settings.isLight ? themes.light : themes.dark;
    term.options.scrollback = s.Scrollback;
    term.options.cursorBlink = s.CursorBlink;
    if (changed) scheduleFit();
  });
  $effect(() => { if (visible && term) { setTimeout(() => { refit(); term.focus(); }, 0); } });

  export function zoom(delta: number) {
    const base = settings.value.FontSize || 14;
    const cur = term.options.fontSize ?? base;
    term.options.fontSize = delta === 0 ? base : Math.max(8, Math.min(40, cur + delta));
    scheduleFit();
  }

  onDestroy(() => { ro?.disconnect(); if (resizeTimer) clearTimeout(resizeTimer); try { term?.dispose(); } catch {} });
</script>

<div class="pane" style:display={visible ? 'flex' : 'none'}>
  <div class="term" bind:this={container}></div>
  {#if tab.monitor}<MonitorBar info={tab.monitor} />{/if}
</div>

<style>
  .pane { position: absolute; inset: 0; display: flex; flex-direction: column; background: var(--terminal-bg); }
  .term { flex: 1; min-height: 0; padding: 2px 4px; }
  :global(.xterm) { height: 100%; }
  :global(.xterm, .xterm *) { font-variant-ligatures: none !important; font-feature-settings: "liga" 0, "calt" 0, "tnum" 1 !important; }
</style>
