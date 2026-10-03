<script lang="ts">
  let { text, right = '' }: { text: string; right?: string } = $props();
</script>

<div class="status">
  <span class="left" title={text}>{text}</span>
  <span class="right">{right}</span>
</div>

<style>
  .status { height: 26px; display: flex; align-items: center; justify-content: space-between; padding: 4px 8px; font-size: 12px; color: var(--theme-fg-muted); background: var(--theme-status-bg); border-top: 1px solid var(--theme-border); }
  .left { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .right { white-space: nowrap; margin-left: 12px; }
</style>
