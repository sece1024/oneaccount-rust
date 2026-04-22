<script lang="ts">
  import { onMount } from 'svelte'
  import { listAccounts, fmtBalance, type Account } from './lib/api'
  import Ledger from './pages/Ledger.svelte'
  import Accounts from './pages/Accounts.svelte'
  import Transactions from './pages/Transactions.svelte'
  import Overview from './pages/Overview.svelte'
  import Settings from './pages/Settings.svelte'
  import Toast from './components/Toast.svelte'
  import ConfirmDialog from './components/ConfirmDialog.svelte'

  type Tab = 'ledger' | 'overview' | 'transactions' | 'accounts' | 'settings'

  const TABS: { id: Tab; label: string; icon: string }[] = [
    { id: 'ledger',       label: '月结表格', icon: '📒' },
    { id: 'overview',     label: '统计概览', icon: '📊' },
    { id: 'transactions', label: '账目流水', icon: '💸' },
    { id: 'accounts',     label: '账户管理', icon: '💳' },
    { id: 'settings',     label: '设置',     icon: '⚙️' },
  ]

  function tabFromHash(): Tab {
    const h = location.hash.replace('#/', '').replace('#', '')
    return TABS.find(t => t.id === h)?.id ?? 'ledger'
  }

  let tab: Tab = tabFromHash()
  let sidebarAccounts: Account[] = []

  $: liquidTotal = sidebarAccounts.filter(a => a.is_liquid).reduce((s, a) => s + a.balance, 0)
  $: netTotal = sidebarAccounts.reduce((s, a) => s + a.balance, 0)

  async function refreshSidebar() {
    try { sidebarAccounts = await listAccounts() } catch {}
  }

  function navigate(id: Tab) {
    tab = id
    location.hash = '#/' + id
    refreshSidebar()
  }

  onMount(() => {
    refreshSidebar()
    const onHash = () => { tab = tabFromHash() }
    window.addEventListener('hashchange', onHash)
    const interval = setInterval(refreshSidebar, 30_000)
    return () => { window.removeEventListener('hashchange', onHash); clearInterval(interval) }
  })
</script>

<Toast />
<ConfirmDialog />
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
    {#if sidebarAccounts.length > 0}
      <div class="asset-summary">
        <div class="summary-row">
          <span class="summary-label">净资产</span>
          <span class="summary-val" class:pos={netTotal >= 0} class:neg={netTotal < 0}>{fmtBalance(netTotal)}</span>
        </div>
        <div class="summary-row">
          <span class="summary-label">活动资金</span>
          <span class="summary-val liquid">{fmtBalance(liquidTotal)}</span>
        </div>
      </div>
    {/if}
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
    {:else if tab === 'settings'}
      <Settings />
    {/if}
  </main>
</div>

<style>
.layout { display: flex; min-height: 100vh; }

.sidebar {
  width: 200px;
  flex-shrink: 0;
  background: linear-gradient(180deg, var(--bg2) 0%, #151823 100%);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 20px 0;
  gap: 4px;
  box-shadow: 2px 0 12px rgba(0,0,0,.3);
}

.brand {
  font-size: 17px;
  font-weight: 800;
  padding: 0 20px 20px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 8px;
  background: linear-gradient(135deg, var(--cyan), #a78bfa);
  -webkit-background-clip: text;
  -webkit-text-fill-color: transparent;
  background-clip: text;
  letter-spacing: 0.5px;
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
.nav-item.active {
  background: linear-gradient(90deg, rgba(125,211,252,.08) 0%, transparent 100%);
  color: var(--cyan);
  border-left: 2px solid var(--cyan);
}
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
  padding: 16px 20px;
  font-size: 11px;
}

.asset-summary {
  margin-top: auto;
  padding: 16px 20px;
  border-top: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.summary-row { display: flex; justify-content: space-between; align-items: center; }
.summary-label { font-size: 11px; color: var(--muted); }
.summary-val { font-size: 13px; font-weight: 600; font-variant-numeric: tabular-nums; }
.summary-val.liquid { color: var(--cyan); }

/* ── Mobile: bottom tab bar ────────────────────────────────────────────────── */
@media (max-width: 768px) {
  .layout { flex-direction: column; }
  .sidebar {
    width: 100%;
    flex-direction: row;
    align-items: center;
    padding: 0;
    gap: 0;
    border-right: none;
    border-bottom: 1px solid var(--border);
    position: fixed;
    bottom: 0;
    left: 0;
    z-index: 100;
    order: 2;
  }
  .brand { display: none; }
  .sidebar-footer { display: none; }
  .asset-summary { display: none; }
  nav { display: flex; width: 100%; }
  .nav-item {
    flex: 1;
    justify-content: center;
    padding: 10px 4px;
    font-size: 11px;
    flex-direction: column;
    gap: 2px;
    text-align: center;
  }
  .nav-item.active { border-left: none; border-top: 2px solid var(--cyan); }
  .icon { font-size: 18px; }
  .content {
    padding: 16px;
    padding-bottom: 72px; /* space for bottom bar */
  }
}
</style>
