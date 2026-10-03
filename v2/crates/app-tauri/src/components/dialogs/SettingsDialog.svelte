<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import type { AppSettings } from '../../lib/types';
  import { settings } from '../../stores/settings.svelte';
  let { onclose }: { onclose: () => void } = $props();
  let s = $state<AppSettings>({ ...settings.value });
  async function browseEditor() {
    try {
      const { open } = await import('@tauri-apps/plugin-dialog');
      const f = await open({ title: t('Str_SelectEditorTitle'), multiple: false, filters: [{ name: 'Executable', extensions: ['exe'] }, { name: t('Str_AllFiles'), extensions: ['*'] }] });
      if (typeof f === 'string') s.CustomEditorPath = f;
    } catch {}
  }
  async function save() {
    const out = { ...s, FontSize: Number(s.FontSize) || 14, KeepAliveSeconds: Math.max(5, Number(s.KeepAliveSeconds) || 5), Scrollback: Math.max(100, Number(s.Scrollback) || 10000), MaxParallelConnects: Math.max(1, Number(s.MaxParallelConnects) || 4), CustomEditorPath: s.CustomEditorPath.trim() };
    await settings.save(out);
    onclose();
  }
</script>

<Modal title={t('Str_SettingsTitle')} width={520} onclose={onclose}>
  <div class="section-title">{t('Str_AppearanceTerminal')}</div>
  <div class="grid">
    <label for="s-lang">{t('Str_LanguageLabel')}</label>
    <select id="s-lang" class="input" bind:value={s.Language}><option value="en">English (Default)</option><option value="vi">Tiếng Việt</option></select>
    <label for="s-font">{t('Str_FontLabel')}</label>
    <select id="s-font" class="input" bind:value={s.FontFamily}><option>JetBrains Mono</option><option>Cascadia Mono</option><option>Consolas</option></select>
    <label for="s-size">{t('Str_FontSizeLabel')}</label>
    <select id="s-size" class="input" bind:value={s.FontSize}>{#each [11, 12, 13, 14, 16, 18, 20] as n}<option value={n}>{n}</option>{/each}</select>
    <label for="s-theme">{t('Str_ThemeLabel')}</label>
    <select id="s-theme" class="input" bind:value={s.Theme}><option>Dark</option><option>Light</option></select>
    <label for="s-sb">{t('Str_Scrollback')}</label>
    <input id="s-sb" class="input" type="number" min="100" bind:value={s.Scrollback} />
    <span></span><label class="check"><input type="checkbox" bind:checked={s.CursorBlink} />{t('Str_CursorBlink')}</label>
  </div>
  <div class="section-title" style="margin-top:14px">{t('Str_ActionsClipboard')}</div>
  <label class="check" style="margin-bottom:8px"><input type="checkbox" bind:checked={s.CopyOnSelect} />{t('Str_CopyOnSelect')}</label>
  <label class="form-label" for="s-rc">{t('Str_RightClickLabel')}</label>
  <select id="s-rc" class="input" bind:value={s.RightClickAction} style="margin-bottom:8px"><option value="Paste">{t('Str_RightClickPaste')}</option><option value="Menu">{t('Str_RightClickMenu')}</option></select>
  <label class="check"><input type="checkbox" bind:checked={s.ConfirmMultilinePaste} />{t('Str_ConfirmMultiline')}</label>
  <div class="section-title" style="margin-top:14px">{t('Str_NetworkSftp')}</div>
  <label class="check" style="margin-bottom:8px"><input type="checkbox" bind:checked={s.ShowHiddenFiles} />{t('Str_ShowHiddenFiles')}</label>
  <label class="form-label" for="s-ed">{t('Str_CustomEditorLabel')}</label>
  <div style="display:flex; gap:4px; margin-bottom:8px"><input id="s-ed" class="input" bind:value={s.CustomEditorPath} /><button class="btn" onclick={browseEditor}>{t('Str_BrowseBtn')}</button></div>
  <div class="grid">
    <label for="s-ka">{t('Str_KeepAliveLabel')}</label><input id="s-ka" class="input" type="number" min="5" bind:value={s.KeepAliveSeconds} style="width:90px; text-align:center" />
    <label for="s-mp">{t('Str_MaxParallel')}</label><input id="s-mp" class="input" type="number" min="1" max="32" bind:value={s.MaxParallelConnects} style="width:90px; text-align:center" />
  </div>
  {#snippet actions()}
    <button class="btn primary" onclick={save}>{t('Str_SaveSettings')}</button>
    <button class="btn" onclick={onclose}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>

<style>
  .grid { display: grid; grid-template-columns: auto 1fr; gap: 8px 12px; align-items: center; }
</style>
