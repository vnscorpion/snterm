<script lang="ts">
  // Thanh "Server Monitor" dưới terminal (như v1 MonitorBar): OS, hostname, CPU, RAM, mạng, uptime, user, đĩa.
  import { untrack } from 'svelte';
  import { t } from '../../lib/i18n.svelte';
  import type { MonitorInfo } from '../../lib/types';
  let { info }: { info: MonitorInfo } = $props();
  let cpuHist = $state<number[]>([]);
  let ramHist = $state<number[]>([]);
  $effect(() => {
    const cpu = info.cpuPercent, ram = info.ramPercent;
    untrack(() => { cpuHist = [...cpuHist, cpu].slice(-8); ramHist = [...ramHist, ram].slice(-8); });
  });
  function points(h: number[]) {
    const w = 40, hh = 14;
    const step = h.length > 1 ? w / (h.length - 1) : w;
    return h.map((v, i) => `${(i * step).toFixed(1)},${(hh - (Math.min(100, Math.max(0, v)) / 100) * (hh - 2) - 1).toFixed(1)}`).join(' ');
  }
  let showOs = $state(false);
  let showDf = $state(false);
</script>

<div class="mon">
  <div class="seg os" onmouseenter={() => (showOs = true)} onmouseleave={() => (showOs = false)} role="note">
    <img src={`/os/os_${info.osGroup || 'linux'}.png`} alt="" width="15" height="15" onerror={(e) => ((e.currentTarget as HTMLImageElement).style.display = 'none')} />
    <span class="host">{info.hostname}</span>
    {#if showOs}
      <div class="tip">
        <div class="tip-title">{t('Str_SystemInfo')}</div>
        <div class="grid">
          <span class="k">{t('Str_HostnameLabel')}</span><span class="v hn">{info.hostname || '--'}</span>
          <span class="k">{t('Str_OsLabel')}</span><span class="v os">{info.osPretty || info.osGroup || '--'}</span>
          <span class="k">{t('Str_KernelLabel')}</span><span class="v kr">{info.kernel || '--'}</span>
          <span class="k">{t('Str_ArchLabel')}</span><span class="v">{info.arch || '--'}</span>
        </div>
      </div>
    {/if}
  </div>
  <div class="seg"><span class="ic" style="color:#FFA726">🖳</span><span>{info.cpuPercent}%</span><svg class="graph cpu" viewBox="0 0 40 14"><polyline points={points(cpuHist)} /></svg></div>
  <div class="seg"><span class="ic" style="color:#42A5F5">💾</span><span>{info.ramText}</span><svg class="graph ram" viewBox="0 0 40 14"><polyline points={points(ramHist)} /></svg></div>
  <div class="seg"><span class="ic" style="color:#00E676; font-weight:bold">⬆</span><span>{info.uploadText}</span></div>
  <div class="seg"><span class="ic" style="color:#29B6F6; font-weight:bold">⬇</span><span>{info.downloadText}</span></div>
  <div class="seg"><span class="ic" style="color:#AB47BC">⏱</span><span>{info.uptimeText}</span></div>
  <div class="seg"><span class="ic" style="color:#FFCA28">👤</span><span>{info.username}</span></div>
  <div class="seg last" onmouseenter={() => (showDf = true)} onmouseleave={() => (showDf = false)} role="note">
    <span class="ic" style="color:#26A69A">🖴</span><span>{info.diskText}</span>
    {#if showDf}
      <div class="tip df">
        <div class="tip-title">{t('Str_DfOutputTitle')}</div>
        <pre>{info.dfOutput || t('Str_NoDfData')}</pre>
      </div>
    {/if}
  </div>
</div>

<style>
  .mon { height: 26px; display: flex; align-items: center; gap: 0; padding: 0 6px; background: var(--monitor-bg); border-top: 1px solid var(--monitor-border); font-size: 11px; color: #e6edf3; overflow-x: auto; overflow-y: visible; white-space: nowrap; }
  .seg { position: relative; display: flex; align-items: center; gap: 5px; padding-right: 8px; margin-right: 8px; border-right: 1px solid var(--monitor-border); height: 100%; }
  .seg.last { border-right: none; }
  .ic { font-size: 11px; }
  .host { color: #4ba3e3; font-weight: 600; }
  .graph { width: 40px; height: 14px; border: 1px solid #1b5e20; background: #0a1f0a; border-radius: 2px; }
  .graph polyline { fill: none; stroke: #00e676; stroke-width: 1.5; }
  .graph.ram { border-color: #1565c0; background: #0a192f; }
  .graph.ram polyline { stroke: #42a5f5; }
  .tip { position: absolute; bottom: 28px; left: 0; z-index: 50; background: #22252a; border: 1px solid #3e4451; padding: 8px 10px; border-radius: 4px; box-shadow: 0 4px 16px rgba(0,0,0,0.5); color: #e6edf3; }
  .tip-title { font-weight: 700; color: #61afef; margin-bottom: 6px; font-size: 12px; }
  .grid { display: grid; grid-template-columns: auto auto; column-gap: 10px; row-gap: 4px; }
  .k { color: #8b949e; }
  .v.hn { color: #fff; font-weight: 600; } .v.os { color: #98c379; } .v.kr { color: #e5c07b; }
  .tip.df { right: 0; left: auto; max-width: 800px; }
  .tip.df pre { margin: 0; max-height: 450px; overflow: auto; font-family: Consolas, 'Cascadia Code', 'JetBrains Mono', monospace; font-size: 11px; color: #d4d4d4; user-select: text; }
</style>
