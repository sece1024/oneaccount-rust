<script lang="ts">
  import { toast, type ToastItem } from '../lib/toast'
  import { fly, fade } from 'svelte/transition'

  let items: ToastItem[] = []
  toast.subscribe(v => items = v)
</script>

{#if items.length > 0}
  <div class="toast-container" aria-live="polite">
    {#each items as item (item.id)}
      <div
        class="toast-item {item.type}"
        in:fly={{ x: 300, duration: 250 }}
        out:fade={{ duration: 150 }}
      >
        <span class="toast-icon">
          {#if item.type === 'success'}✓{:else if item.type === 'error'}✕{:else}ℹ{/if}
        </span>
        <span class="toast-msg">{item.message}</span>
        <button class="toast-close" on:click={() => toast.dismiss(item.id)}>×</button>
      </div>
    {/each}
  </div>
{/if}

<style>
.toast-container {
  position: fixed;
  top: 16px;
  right: 16px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 380px;
}
.toast-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 8px;
  font-size: 13px;
  box-shadow: 0 4px 20px rgba(0,0,0,.4);
  backdrop-filter: blur(8px);
}
.toast-item.success { background: rgba(74,222,128,.15); border: 1px solid var(--green); color: var(--green); }
.toast-item.error   { background: rgba(248,113,113,.15); border: 1px solid var(--red); color: var(--red); }
.toast-item.info    { background: rgba(125,211,252,.15); border: 1px solid var(--cyan); color: var(--cyan); }
.toast-icon { font-size: 16px; font-weight: 700; flex-shrink: 0; }
.toast-msg { flex: 1; color: var(--text); }
.toast-close {
  background: none; border: none; color: inherit; font-size: 16px;
  cursor: pointer; padding: 0 2px; opacity: .6;
}
.toast-close:hover { opacity: 1; }
</style>
