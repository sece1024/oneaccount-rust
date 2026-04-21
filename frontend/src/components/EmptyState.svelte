<script lang="ts">
  export let icon: string = '📭'
  export let title: string
  export let description: string = ''
  export let action: string = ''
  export let onAction: (() => void) | null = null
</script>

<div class="empty-state">
  <span class="icon">{icon}</span>
  <h3>{title}</h3>
  {#if description}<p class="desc">{description}</p>{/if}
  {#if action && onAction}
    <button class="primary" on:click={onAction}>{action}</button>
  {/if}
</div>

<style>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: 48px 24px;
  text-align: center;
  color: var(--muted);
}
.icon { font-size: 40px; margin-bottom: 12px; }
h3 { margin: 0 0 8px; font-size: 16px; color: var(--text); font-weight: 600; }
.desc { margin: 0 0 16px; font-size: 13px; line-height: 1.6; max-width: 360px; }
</style>
