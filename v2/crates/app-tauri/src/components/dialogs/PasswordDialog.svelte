<script lang="ts">
  import Modal from '../Modal.svelte';
  import { t } from '../../lib/i18n.svelte';
  import type { DialogRequest } from '../../lib/types';
  let { req, onanswer }: { req: DialogRequest; onanswer: (password: string | null, save: boolean) => void } = $props();
  let password = $state('');
  let save = $state(true);
  let show = $state(false);
  function focusEl(el: HTMLElement) { setTimeout(() => el.focus(), 0); }
</script>

<Modal title={t('Str_PasswordTitleFmt', req.title)} width={420} onclose={() => onanswer(null, false)}>
  <div style="font-weight:700; font-size:15px">{t('Str_PasswordTitleFmt', req.title)}</div>
  <div class="muted" style="margin-bottom:10px">{req.username}@{req.host}</div>
  {#if req.wasRejected}<div style="color:#f04438; margin-bottom:8px">{t('Str_WrongSavedPassword')}</div>{/if}
  <label class="form-label" for="pw">{t('Str_PasswordLabel')}</label>
  <div style="display:flex; gap:4px; margin-bottom:10px">
    <input id="pw" class="input" type={show ? 'text' : 'password'} bind:value={password} use:focusEl onkeydown={(e) => { if (e.key === 'Enter') onanswer(password, save); }} />
    <button class="btn" style="width:34px; padding:0" onclick={() => (show = !show)} title="👁">👁</button>
  </div>
  <label class="check"><input type="checkbox" bind:checked={save} />{t('Str_SavePasswordCheck')}</label>
  {#snippet actions()}
    <button class="btn primary" onclick={() => onanswer(password, save)}>{t('Str_Login')}</button>
    <button class="btn" onclick={() => onanswer(null, false)}>{t('Str_Cancel')}</button>
  {/snippet}
</Modal>
