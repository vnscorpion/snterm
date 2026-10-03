import { invoke, listen } from '../lib/ipc';
import { setLanguage } from '../lib/i18n.svelte';
import type { AppSettings } from '../lib/types';

class SettingsStore {
  value = $state<AppSettings>({
    FontFamily: 'JetBrains Mono', FontSize: 14, Language: 'en', Theme: 'Dark', CopyOnSelect: true, RightClickAction: 'Paste',
    ConfirmMultilinePaste: true, Scrollback: 10000, CursorBlink: true, KeepAliveSeconds: 5, ShowHiddenFiles: true, CustomEditorPath: '',
    CollapsedGroups: [], LastExportFolder: '', LastImportFolder: '', MaxParallelConnects: 4, WindowWidth: 1100, WindowHeight: 700, LeftColumnWidth: 400,
  });
  loaded = $state(false);

  async load() {
    this.apply(await invoke<AppSettings>('get_settings'));
    this.loaded = true;
    await listen('settings:changed', (p) => this.apply(p as AppSettings));
  }
  apply(s: AppSettings) {
    this.value = s;
    setLanguage(s.Language);
    document.documentElement.dataset.theme = s.Theme?.toLowerCase() === 'light' ? 'light' : 'dark';
  }
  async save(s: AppSettings) {
    this.apply(await invoke<AppSettings>('save_settings', { settings: s }));
  }
  get isLight() { return this.value.Theme?.toLowerCase() === 'light'; }
}
export const settings = new SettingsStore();
