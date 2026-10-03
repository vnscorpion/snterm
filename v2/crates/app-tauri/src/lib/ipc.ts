// Bọc invoke/listen của Tauri. Khi chạy trong trình duyệt thường (dev/kiểm thử bố cục) dùng mock trong bộ nhớ.
import type { AppInfo, AppSettings, ConnectionStatus, ExportOutcome, ImportFileInfo, ImportResult, SessionDraft, SessionView, SftpListing } from './types';

type Listener = (payload: unknown) => void;
export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

let tauriInvoke: ((cmd: string, args?: Record<string, unknown>) => Promise<unknown>) | null = null;
let tauriListen: ((event: string, cb: (e: { payload: unknown }) => void) => Promise<() => void>) | null = null;
let ChannelCtor: (new () => { onmessage: (data: ArrayBuffer | Uint8Array) => void }) | null = null;

export async function initIpc(): Promise<void> {
  if (!isTauri) return;
  const core = await import('@tauri-apps/api/core');
  const event = await import('@tauri-apps/api/event');
  tauriInvoke = core.invoke as typeof tauriInvoke;
  tauriListen = event.listen as unknown as typeof tauriListen;
  ChannelCtor = core.Channel as unknown as typeof ChannelCtor;
}

export async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (tauriInvoke) return tauriInvoke(cmd, args) as Promise<T>;
  return mockInvoke(cmd, args ?? {}) as Promise<T>;
}

const mockListeners = new Map<string, Set<Listener>>();
export async function listen(event: string, cb: Listener): Promise<() => void> {
  if (tauriListen) return tauriListen(event, (e) => cb(e.payload));
  if (!mockListeners.has(event)) mockListeners.set(event, new Set());
  mockListeners.get(event)!.add(cb);
  return () => mockListeners.get(event)?.delete(cb);
}
export function mockEmit(event: string, payload: unknown) {
  mockListeners.get(event)?.forEach((cb) => cb(payload));
}

/** Kênh nhận bytes thô của terminal. */
export function createOutputChannel(onData: (bytes: Uint8Array) => void): unknown {
  if (ChannelCtor) {
    const ch = new ChannelCtor();
    ch.onmessage = (data) => onData(data instanceof Uint8Array ? data : new Uint8Array(data as ArrayBuffer));
    return ch;
  }
  const mock = { onmessage: (data: ArrayBuffer | Uint8Array) => onData(data instanceof Uint8Array ? data : new Uint8Array(data)) };
  return mock;
}

export function reportError(message: string) {
  console.error(message);
  if (tauriInvoke) tauriInvoke('log_frontend_error', { message }).catch(() => {});
}

// ---------------- Mock backend (trình duyệt) ----------------
const LS_KEY = 'snterm.mock.sessions';
const LS_SETTINGS = 'snterm.mock.settings';
function mockSessions(): SessionView[] {
  try { const raw = localStorage.getItem(LS_KEY); if (raw) return JSON.parse(raw); } catch {}
  const seed: SessionView[] = [
    mk('web-01', 'Dev', '10.0.0.5', 22, 'ubuntu', true),
    mk('db-02', 'Dev', '10.0.0.6', 22, 'root', true),
    mk('api-01', 'Prod', 'api.example.com', 2222, 'deploy', false),
    mk('Máy chủ tiếng Việt', 'Prod', '10.0.1.9', 22, 'admin', true),
    mk('', '', '192.168.1.20', 22, 'pi', false),
  ];
  seed[0].lastConnectedAt = new Date(Date.now() - 3600_000).toISOString();
  seed[2].keyFilePath = 'C:\\keys\\id_ed25519'; seed[2].keyFileMissing = true;
  localStorage.setItem(LS_KEY, JSON.stringify(seed));
  return seed;
}
function mk(name: string, group: string, host: string, port: number, username: string, hasPassword: boolean): SessionView {
  return {
    id: crypto.randomUUID(), name, group, host, port, username, savePassword: true, hasPassword, hasPassphrase: false,
    keyFilePath: null, keyFileMissing: false, createdAt: new Date().toISOString(), lastConnectedAt: null,
    displayName: name || `${username}@${host}`, subtitle: `${username}@${host}`, effectiveGroup: group || 'Chưa phân nhóm',
  };
}
function saveMock(list: SessionView[]) { localStorage.setItem(LS_KEY, JSON.stringify(list)); }
const mockTabs = new Map<string, { status: ConnectionStatus; ch: { onmessage: (d: Uint8Array) => void } | null; sessionId: string; timer?: number }>();
const defaultSettings: AppSettings = {
  FontFamily: 'JetBrains Mono', FontSize: 14, Language: 'vi', Theme: 'Dark', CopyOnSelect: true, RightClickAction: 'Paste',
  ConfirmMultilinePaste: true, Scrollback: 10000, CursorBlink: true, KeepAliveSeconds: 5, ShowHiddenFiles: true, CustomEditorPath: '',
  CollapsedGroups: [], LastExportFolder: '', LastImportFolder: '', MaxParallelConnects: 4, WindowWidth: 1100, WindowHeight: 700, LeftColumnWidth: 400,
};
const enc = new TextEncoder();
async function mockInvoke(cmd: string, args: Record<string, unknown>): Promise<unknown> {
  await new Promise((r) => setTimeout(r, 10));
  switch (cmd) {
    case 'get_app_info': return { version: '2.0.0-mock', dataDir: '%APPDATA%\\SNTerm', localDir: '', logsDir: '', backupsDir: '', platform: 'browser' } as AppInfo;
    case 'get_settings': { try { const raw = localStorage.getItem(LS_SETTINGS); if (raw) return { ...defaultSettings, ...JSON.parse(raw) }; } catch {} return defaultSettings; }
    case 'save_settings': { localStorage.setItem(LS_SETTINGS, JSON.stringify(args.settings)); mockEmit('settings:changed', args.settings); return args.settings; }
    case 'save_window_state': case 'save_collapsed_groups': case 'log_frontend_error': case 'open_path': return null;
    case 'list_sessions': return { sessions: mockSessions(), recoveredFromCorruption: false };
    case 'save_session': {
      const d = args.draft as SessionDraft; const list = mockSessions();
      const idx = d.id ? list.findIndex((s) => s.id === d.id) : -1;
      const base = idx >= 0 ? list[idx] : mk('', '', '', 22, '', false);
      const s: SessionView = { ...base, name: d.name.trim(), group: d.group.trim(), host: d.host.trim(), port: d.port, username: d.username.trim(),
        savePassword: d.savePassword, hasPassword: d.savePassword && (!!d.password || (base.hasPassword && !d.clearPassword)),
        keyFilePath: d.keyFilePath || null, keyFileMissing: false };
      s.displayName = s.name || `${s.username}@${s.host}`; s.subtitle = `${s.username}@${s.host}`; s.effectiveGroup = s.group || 'Chưa phân nhóm';
      if (idx >= 0) list[idx] = s; else list.push(s);
      saveMock(list); return s;
    }
    case 'delete_sessions': { const ids = args.ids as string[]; const list = mockSessions().filter((s) => !ids.includes(s.id)); saveMock(list); return ids.length; }
    case 'duplicate_session': { const list = mockSessions(); const s = list.find((x) => x.id === args.id)!; const c = { ...s, id: crypto.randomUUID(), name: s.name ? `${s.name} (bản sao)` : '', lastConnectedAt: null }; c.displayName = c.name || c.subtitle; list.push(c); saveMock(list); return c; }
    case 'move_sessions_to_group': { const ids = args.ids as string[]; const g = (args.group as string).trim(); const list = mockSessions(); for (const s of list) if (ids.includes(s.id)) { s.group = g; s.effectiveGroup = g || 'Chưa phân nhóm'; } saveMock(list); return null; }
    case 'rename_group': { const list = mockSessions(); for (const s of list) if (s.effectiveGroup.toLowerCase() === (args.oldName as string).toLowerCase()) { s.group = (args.newName as string).trim(); s.effectiveGroup = s.group || 'Chưa phân nhóm'; } saveMock(list); return null; }
    case 'inspect_key_file': throw 'notfound';
    case 'clipboard_write': { try { await navigator.clipboard.writeText(args.text as string); } catch {} return null; }
    case 'clipboard_read': { try { return await navigator.clipboard.readText(); } catch { return ''; } }
    case 'connected_tab_count': return [...mockTabs.values()].filter((t) => t.status === 'connected').length;
    case 'test_connection': { await new Promise((r) => setTimeout(r, 800)); return 'ok'; }
    case 'dialog_answer': return null;
    case 'terminal_open': {
      const id = crypto.randomUUID(); const ch = args.onOutput as { onmessage: (d: Uint8Array) => void };
      const sessionId = args.sessionId as string;
      mockTabs.set(id, { status: 'connecting', ch, sessionId });
      setTimeout(() => {
        const t = mockTabs.get(id); if (!t) return;
        t.status = 'connected';
        mockEmit('tab:status', { tabId: id, sessionId, status: 'connected', message: '' });
        mockEmit('tab:connected', id);
        ch.onmessage(enc.encode(`\x1b[32mMock shell\x1b[0m — gõ thử, Enter để xuống dòng.\r\n\x1b[1;34muser@mock\x1b[0m:\x1b[1;32m~\x1b[0m$ `));
        mockEmit('tab:monitor', { tabId: id, info: { osGroup: 'ubuntu', hostname: 'mock-host', cpuPercent: 12, ramPercent: 40, ramText: '0.80 GB / 2.00 GB', uploadText: '0.3 KB/s', downloadText: '1.2 KB/s', uptimeText: '3 days', username: 'root', diskText: '/: 45%', osPretty: 'Ubuntu 22.04 LTS', kernel: '5.15.0', arch: 'x86_64', dfOutput: 'Filesystem Size Used Avail Use% Mounted on\n/dev/sda1 40G 18G 22G 45% /' } });
      }, 400);
      return id;
    }
    case 'terminal_input': { const t = mockTabs.get(args.tabId as string); if (!t?.ch) return null; const d = args.data as string; if (t.status !== 'connected') { if (/[r\r\n]/i.test(d)) { t.status = 'connected'; mockEmit('tab:status', { tabId: args.tabId, sessionId: t.sessionId, status: 'connected', message: '' }); t.ch.onmessage(enc.encode('\r\nReconnected.\r\n$ ')); } return null; } if (d === '\r') t.ch.onmessage(enc.encode('\r\n$ ')); else if (d === '\x7f') t.ch.onmessage(enc.encode('\b \b')); else t.ch.onmessage(enc.encode(d)); return null; }
    case 'terminal_resize': case 'terminal_set_visible': return null;
    case 'terminal_reconnect': { const t = mockTabs.get(args.tabId as string); if (t) { t.status = 'connected'; mockEmit('tab:status', { tabId: args.tabId, sessionId: t.sessionId, status: 'connected', message: '' }); } return null; }
    case 'terminal_close': { mockTabs.delete(args.tabId as string); return null; }
    case 'sftp_list': {
      const path = (args.path as string) || '/home/ubuntu';
      const items = [{ name: '..', fullName: path.split('/').slice(0, -1).join('/') || '/', isDirectory: true, isParentDirectory: true, isSymbolicLink: false, size: 0, lastModified: 0, permissions: '', mode: 0, userId: 0, groupId: 0, owner: '', group: '' }];
      for (const [n, d, sz] of [['src', true, 0], ['nginx', true, 0], ['báo_cáo.txt', false, 12288], ['app.log', false, 5_400_000], ['.bashrc', false, 3771]] as [string, boolean, number][]) {
        if (!(args.showHidden as boolean) && n.startsWith('.')) continue;
        items.push({ name: n, fullName: `${path}/${n}`, isDirectory: d, isParentDirectory: false, isSymbolicLink: false, size: sz, lastModified: Math.floor(Date.now() / 1000) - 86400, permissions: d ? 'drwxr-xr-x' : '-rw-r--r--', mode: d ? 0o755 : 0o644, userId: 1000, groupId: 1000, owner: 'ubuntu', group: 'ubuntu' });
      }
      return { path, home: '/home/ubuntu', items } as SftpListing;
    }
    case 'sftp_is_dir': return true;
    case 'sftp_mkdir': case 'sftp_rename': case 'sftp_remove': case 'sftp_chmod': case 'sftp_cancel_transfer': return null;
    case 'sftp_upload': case 'sftp_download': return [];
    case 'sftp_open_in_editor': return '/tmp/mock';
    case 'export_sessions': return { exported: (args.ids as string[]).length, path: args.path, messages: [] } as ExportOutcome;
    case 'import_inspect': return { path: args.path, fileName: 'mock.snterm', format: 'snterm-sessions', count: 2, protected: true, items: [
      { id: '1', name: 'web-01', subtitle: 'ubuntu@10.0.0.5:22', group: 'Dev', status: 'duplicate', existingName: 'web-01', hasSecrets: true, keyFileName: null },
      { id: '2', name: 'new-vm', subtitle: 'root@10.0.0.9:22', group: 'Dev', status: 'new', existingName: null, hasSecrets: true, keyFileName: null }] } as ImportFileInfo;
    case 'import_sessions': return { totalInFile: 2, importedCount: 1, skippedCount: 1, overwrittenCount: 0, addedCopyCount: 0, corruptSecretsCount: 0, messages: [] } as ImportResult;
    case 'find_mobaxterm_candidate': return null;
    default: throw new Error(`mock: unknown command ${cmd}`);
  }
}
