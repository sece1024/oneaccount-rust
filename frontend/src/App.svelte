<script lang="ts">
  import { onMount } from 'svelte'
  import Ledger from './pages/Ledger.svelte'
  import Accounts from './pages/Accounts.svelte'
  import Transactions from './pages/Transactions.svelte'
  import Overview from './pages/Overview.svelte'
  import Toast from './components/Toast.svelte'

  type Tab = 'ledger' | 'overview' | 'transactions' | 'accounts'

  const TABS: { id: Tab; label: string; icon: string }[] = [
    { id: 'ledger',       label: '月结表格', icon: '📒' },
    { id: 'overview',     label: '统计概览', icon: '📊' },
    { id: 'transactions', label: '账目流水', icon: '💸' },
    { id: 'accounts',     label: '账户管理', icon: '💳' },
  ]

  function tabFromHash(): Tab {
    const h = location.hash.replace('#/', '').replace('#', '')
    return TABS.find(t => t.id === h)?.id ?? 'ledger'
  }

  let tab: Tab = tabFromHash()

  function navigate(id: Tab) {
    tab = id
    location.hash = '#/' + id
  }

  onMount(() => {
    const onHash = () => { tab = tabFromHash() }
    window.addEventListener('hashchange', onHash)
    return () => window.removeEventListener('hashchange', onHash)
  })
</script>

<Toast />
<div class="layout">
  <aside class="sidebar">
    <div class="brand">OneAccount</div>
    <nav>
      {#each TABS as t}
        <button
          class="nav-item"
          class:active={tab === t.id}
          on:click={() => navigate(t.id)}
        >
          <span class="icon">{t.icon}</span>
          {t.label}
        </button>
      {/each}
    </nav>
    <div class="sidebar-footer muted">
      API: localhost:8080
    </div>
  </aside>

  <main class="content">
    {#if tab === 'ledger'}
      <Ledger />
    {:else if tab === 'overview'}
      <Overview />
    {:else if tab === 'transactions'}
      <Transactions />
    {:else if tab === 'accounts'}
      <Accounts />
    {/if}
  </main>
</div>

<style>
.layout { display: flex; min-height: 100vh; }

.sidebar {
  width: 200px;
  flex-shrink: 0;
  background: var(--bg2);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 20px 0;
  gap: 4px;
}

.brand {
  font-size: 16px;
  font-weight: 700;
  color: var(--cyan);
  padding: 0 20px 20px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 8px;
}

.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  border-radius: 0;
  padding: 10px 20px;
  background: transparent;
  color: var(--muted);
  font-size: 14px;
  text-align: left;
}
.nav-item:hover { background: var(--bg3); color: var(--text); }
.nav-item.active { background: var(--bg3); color: var(--cyan); border-left: 2px solid var(--cyan); }
.icon { font-size: 16px; }

.content {
  flex: 1;
  padding: 28px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.sidebar-footer {
  margin-top: auto;
  padding: 16px 20px;
  font-size: 11px;
}
</style>
