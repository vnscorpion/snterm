<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import { ui } from '../../stores/ui.svelte';
  let { target, currentPerms, hasDirectory, onok, oncancel }: { target: string; currentPerms: string; hasDirectory: boolean; onok: (mode: number, recursive: boolean) => void; oncancel: () => void } = $props();
  function fromPerms(p: string): string {
    if (!p || p.length < 10) return '755';
    const o = (p[1] === 'r' ? 4 : 0) + (p[2] === 'w' ? 2 : 0) + (p[3] === 'x' || p[3] === 's' ? 1 : 0);
    const g = (p[4] === 'r' ? 4 : 0) + (p[5] === 'w' ? 2 : 0) + (p[6] === 'x' || p[6] === 's' ? 1 : 0);
    const a = (p[7] === 'r' ? 4 : 0) + (p[8] === 'w' ? 2 : 0) + (p[9] === 'x' || p[9] === 't' ? 1 : 0);
    return `${o}${g}${a}`;
  }
  let octal = $state(fromPerms(currentPerms));
  let recursive = $state(false);
  const bits = $derived.by(() => { let s = octal.trim(); if (s.length === 4 && s.startsWith('0')) s = s.slice(1); if (!/^[0-7]{3}$/.test(s)) return null; return s.split('').map((c) => parseInt(c, 10)); });
  function setBit(who: number, mask: number, on: boolean) {
    const b = bits ?? [0, 0, 0];
    b[who] = on ? b[who] | mask : b[who] & ~mask;
    octal = b.join('');
  }
  function ok() {
    let s = octal.trim(); if (s.length === 4 && s.startsWith('0')) s = s.slice(1);
    if (!/^[0-7]{3}$/.test(s)) { ui.message(t('Str_InputErrorTitle'), t('Str_InvalidOctal'), 'warn'); return; }
    onok(parseInt(s, 8), recursive);
  }
  const subjects = [t('Str_OwnerSubject'), t('Str_GroupSubject'), t('Str_OthersSubject')];
</script>

<Modal title={t('Str_ChmodTitle')} width={420} onclose={oncancel}>
  <div style="margin-bottom:10px">{t('Str_Item')}<b>{target}</b></div>
  <table class="perm">
    <thead><tr><th>{t('Str_Subject')}</th><th>{t('Str_Read')}</th><th>{t('Str_Write')}</th><th>{t('Str_Execute')}</th></tr></thead>
    <tbody>
      {#each subjects as sub, who}
        <tr><td>{sub}</td>
          {#each [4, 2, 1] as mask}
            <td><input type="checkbox" checked={!!bits && (bits[who] & mask) !== 0} onchange={(e) => setBit(who, mask, (e.currentTarget as HTMLInputElement).checked)} /></td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
  <div style="display:flex; align-items:center; gap:8px; margin-top:12px">
    <span>{t('Str_OctalLabel')}</span>
    <input class="input" style="width:90px; text-align:center; font-family:Consolas, monospace" bind:value={octal} maxlength="4" onkeydown={(e) => { if (e.key === 'Enter') ok(); }} />
  </div>
  {#if hasDirectory}<label class="check" style="margin-top:10px"><input type="checkbox" bind:checked={recursive} />{t('Str_RecursiveLabel')}</label>{/if}
  {#snippet actions()}
    <button class="btn primary" onclick={ok}>{t('Str_Ok')}</button>
    <button class="btn" onclick={oncancel}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>

<style>
  .perm { border-collapse: collapse; width: 100%; }
  .perm th, .perm td { padding: 5px 8px; text-align: center; border-bottom: 1px solid var(--theme-border); }
  .perm th:first-child, .perm td:first-child { text-align: left; }
</style>
