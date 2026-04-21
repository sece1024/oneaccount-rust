<script lang="ts">
  import { confirmState } from '../lib/confirm'
  import { fly, fade } from 'svelte/transition'

  function handle(result: boolean) {
    confirmState.update(s => {
      s.resolve?.(result)
      return { ...s, open: false, resolve: null }
    })
  }
</script>

{#if $confirmState.open}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div class="overlay" transition:fade={{ duration: 150 }} on:click={() => handle(false)}>
    <div class="dialog" transition:fly={{ y: -20, duration: 200 }} on:click|stopPropagation>
      <h3>{$confirmState.title}</h3>
      <p>{$confirmState.message}</p>
      <div class="actions">
        <button on:click={() => handle(false)}>取消</button>
        <button class:danger={$confirmState.danger} class:primary={!$confirmState.danger} on:click={() => handle(true)}>
          {$confirmState.confirmText}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2000;
}
.dialog {
  background: var(--bg2);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 24px;
  min-width: 320px;
  max-width: 420px;
}
h3 { margin: 0 0 8px; font-size: 16px; }
p { margin: 0 0 20px; font-size: 14px; color: var(--muted); line-height: 1.5; }
.actions { display: flex; justify-content: flex-end; gap: 8px; }
.danger {
  background: var(--red);
  color: #fff;
  border: none;
  padding: 6px 16px;
  border-radius: 4px;
  cursor: pointer;
}
.danger:hover { opacity: 0.85; }
</style>
