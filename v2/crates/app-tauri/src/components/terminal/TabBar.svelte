<script lang="ts">
  import { tabs, type Tab } from '../../stores/tabs.svelte';
  import { ui } from '../../stores/ui.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { invoke } from '../../lib/ipc';

  let { onopenlist, onclose }: { onopenlist: () => void; onclose: (tab: Tab) => void } = $props();
  let scroller: HTMLDivElement | undefined = $state();
  let dragFrom = $state<number | null>(null);
  let dragOver = $state<number | null>(null);

  function dot(status: string) {
    return status === 'connected' ? 'var(--status-connected)' : status === 'connecting' || status === 'reconnecting' ? 'var(--status-connecting)' : 'var(--status-off)';
  }
  function onWheel(e: WheelEvent) { if (scroller && e.deltaY !== 0) { scroller.scrollLeft += e.deltaY; e.preventDefault(); } }
  function scrollBy(dx: number) { scroller?.scrollBy({ left: dx, behavior: 'smooth' }); }
  function onMouseDown(e: MouseEvent, tab: Tab) {
    if (e.button === 1) { e.preventDefault(); onclose(tab); return; }
    if (e.button === 0) tabs.select(tab.localId);
  }
  function onContext(e: MouseEvent, tab: Tab) {
    ui.openMenu(e, [
      { label: t('Str_Reconnect'), action: () => { if (tab.id) invoke('terminal_reconnect', { tabId: tab.id }); } },
      { label: t('Str_DuplicateTab'), action: () => tabs.open(tab.session) },
      { label: t('Str_RenameTab'), action: async () => { const v = await ui.input(t('Str_RenameTab'), t('Str_RenameTabPrompt'), tab.title); if (v) tabs.rename(tab.localId, v); } },
      { sep: true, label: '' },
      { label: t('Str_CloseTab'), shortcut: 'Ctrl+Shift+W', action: () => onclose(tab) },
      { label: t('Str_CloseOtherTabs'), action: () => tabs.closeOthers(tab.localId) },
      { label: t('Str_CloseRightTabs'), action: () => tabs.closeRight(tab.localId) },
      { label: t('Str_CloseAllTabs'), action: () => tabs.closeAll() },
    ]);
  }
  function listMenu(e: MouseEvent) {
    const items = tabs.list.map((tb) => ({ label: `${tb.status === 'connected' ? '●' : tb.status === 'connecting' ? '◐' : '○'} ${tb.title}`, action: () => tabs.select(tb.localId) }));
    ui.openMenu(e, items.length ? items : [{ label: t('Str_NoVmConnected'), disabled: true }]);
  }
  $effect(() => {
    const id = tabs.selectedLocalId;
    if (!id || !scroller) return;
    scroller.querySelector(`[data-local="${id}"]`)?.scrollIntoView({ inline: 'nearest', block: 'nearest' });
  });
</script>

<div class="bar">
  <button class="nav" onclick={() => scrollBy(-200)} title="◀">◀</button>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scroller" bind:this={scroller} onwheel={onWheel}>
    {#each tabs.list as tab, i (tab.localId)}
      <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
      <div class="tab" class:selected={tabs.selectedLocalId === tab.localId} class:dragover={dragOver === i} data-local={tab.localId}
           title={tab.session.subtitle} draggable="true"
           onmousedown={(e) => onMouseDown(e, tab)} oncontextmenu={(e) => onContext(e, tab)}
           ondragstart={(e) => { dragFrom = i; e.dataTransfer?.setData('text/plain', String(i)); }}
           ondragover={(e) => { e.preventDefault(); dragOver = i; }} ondragleave={() => (dragOver = null)}
           ondrop={(e) => { e.preventDefault(); if (dragFrom !== null) tabs.move(dragFrom, i); dragFrom = null; dragOver = null; }}
           ondragend={() => { dragFrom = null; dragOver = null; }}>
        <span class="dot" style="background:{dot(tab.status)}"></span>
        <span class="title">{tab.title}</span>
        <button class="close" title={t('Str_CloseTab')} onmousedown={(e) => e.stopPropagation()} onclick={(e) => { e.stopPropagation(); onclose(tab); }}>×</button>
      </div>
    {/each}
    <button class="plus" title={t('Str_OpenVmListTooltip')} onclick={onopenlist}>+</button>
  </div>
  <button class="nav" onclick={listMenu} title={t('Str_Tabs')}>▾</button>
  <button class="nav" onclick={() => scrollBy(200)} title="▶">▶</button>
</div>

<style>
  .bar { height: 36px; display: flex; align-items: stretch; background: var(--theme-tabbar-bg); border-bottom: 1px solid var(--theme-tabbar-border); }
  .nav { width: 22px; border: none; background: transparent; color: var(--theme-tab-unselected-fg); cursor: pointer; font-size: 10px; }
  .nav:hover { color: var(--theme-tab-selected-fg); background: var(--theme-button-hover); }
  .scroller { flex: 1; display: flex; align-items: flex-end; overflow-x: auto; overflow-y: hidden; scrollbar-width: none; padding-top: 4px; }
  .scroller::-webkit-scrollbar { display: none; }
  .tab { display: flex; align-items: center; gap: 7px; margin: 0 2px; padding: 6px 8px 6px 10px; border-radius: 6px 6px 0 0; background: var(--theme-tab-unselected-bg); color: var(--theme-tab-unselected-fg); cursor: pointer; max-width: 220px; min-width: 90px; height: 30px; white-space: nowrap; border: 1px solid transparent; border-bottom: none; }
  .tab.selected { background: var(--theme-tab-selected-bg); color: var(--theme-tab-selected-fg); border-color: var(--theme-border); }
  .tab.dragover { outline: 2px dashed var(--theme-button-hover-border); }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
  .title { overflow: hidden; text-overflow: ellipsis; flex: 1; }
  .close { border: none; background: transparent; color: inherit; cursor: pointer; font-size: 15px; line-height: 1; padding: 0 2px; border-radius: 3px; opacity: 0.7; }
  .close:hover { opacity: 1; background: rgba(255,255,255,0.15); }
  .plus { border: none; background: transparent; color: var(--theme-tab-unselected-fg); font-size: 18px; cursor: pointer; padding: 0 10px; height: 30px; }
  .plus:hover { color: var(--theme-tab-selected-fg); }
</style>
