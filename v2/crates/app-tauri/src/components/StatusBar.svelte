<script lang="ts">
  let { text, right = '', onrightclick, rightError = false }: { text: string; right?: string; onrightclick?: () => void; rightError?: boolean } = $props();
</script>

<div class="status">
  <span class="left" title={text}>{text}</span>
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <span class="right" class:err={rightError} class:link={!!onrightclick && !!right} onclick={() => onrightclick?.()}>{right}</span>
</div>

<style>
  .status { height: 26px; display: flex; align-items: center; justify-content: space-between; padding: 4px 8px; font-size: 12px; color: var(--theme-fg-muted); background: var(--theme-status-bg); border-top: 1px solid var(--theme-border); }
  .left { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .right { white-space: nowrap; margin-left: 12px; }
  .right.link { cursor: pointer; }
  .right.link:hover { text-decoration: underline; }
  .right.err { color: var(--danger); }
</style>
