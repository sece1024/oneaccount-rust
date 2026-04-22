<script lang="ts">
  import { onMount } from 'svelte'
  import { toast } from '../lib/toast'
  import EmptyState from '../components/EmptyState.svelte'
  import { confirm } from '../lib/confirm'
  import {
    listAccounts, createAccount, deleteAccount, updateAccountLiquid,
    fmtBalance, ACCOUNT_TYPES, ACCOUNT_TYPE_LABELS,
    type Account,
  } from '../lib/api'

  let accounts: Account[] = []
  let loading = true
  let error = ''
  let showForm = false
  let submitting = false

  let form = { name: '', account_type: 'bank', currency: 'CNY', initial_balance: 0, is_liquid: true }
  let formEl: HTMLElement

  async function load() {
    loading = true; error = ''
    try { accounts = await listAccounts() }
    catch (e: any) { error = e.message }
    finally { loading = false }
  }

  async function submit() {
    if (!form.name.trim()) return
    submitting = true
    try {
      await createAccount({ ...form })
      showForm = false
      form = { name: '', account_type: 'bank', currency: 'CNY', initial_balance: 0, is_liquid: true }
      toast.success('账户已创建')
      await load()
    } catch (e: any) { error = e.message }
    finally { submitting = false }
  }

  async function del(id: number, name: string) {
    const ok = await confirm({ message: `确认删除账户「${name}」？关联的快照数据也将丢失。`, confirmText: '删除' })
    if (!ok) return
    try { await deleteAccount(id); toast.success('账户已删除'); await load() }
    catch (e: any) { error = e.message; toast.error(e.message) }
  }

  async function toggleLiquid(acc: Account) {
    try {
      await updateAccountLiquid(acc.id, !acc.is_liquid)
      toast.success(`已切换为${!acc.is_liquid ? '活动' : '非活动'}资金`)
      await load()
    } catch (e: any) { error = e.message; toast.error(e.message) }
  }

  const liquidTotal = () => accounts.filter(a => a.is_liquid).reduce((s, a) => s + a.balance, 0)
  const illiquidTotal = () => accounts.filter(a => !a.is_liquid).reduce((s, a) => s + a.balance, 0)
  const total = () => accounts.reduce((s, a) => s + a.balance, 0)

  type SortKey = 'name' | 'account_type' | 'balance' | 'is_liquid'
  let sortKey: SortKey = 'name'
  let sortAsc = true

  function toggleSort(key: SortKey) {
    if (sortKey === key) sortAsc = !sortAsc
    else { sortKey = key; sortAsc = true }
  }

  $: sortedAccounts = [...accounts].sort((a, b) => {
    let cmp = 0
    if (sortKey === 'name') cmp = a.name.localeCompare(b.name)
    else if (sortKey === 'account_type') cmp = a.account_type.localeCompare(b.account_type)
    else if (sortKey === 'balance') cmp = a.balance - b.balance
    else if (sortKey === 'is_liquid') cmp = (a.is_liquid === b.is_liquid ? 0 : a.is_liquid ? -1 : 1)
    return sortAsc ? cmp : -cmp
  })

  function sortIcon(key: SortKey) {
    if (sortKey !== key) return '↕'
    return sortAsc ? '↑' : '↓'
  }

  onMount(load)
</script>

<div class="accounts">
  <div class="toolbar">
    <div class="totals">
      <span class="muted">活动资金 <strong class:neg={liquidTotal() < 0} class:pos={liquidTotal() >= 0}>{fmtBalance(liquidTotal())}</strong></span>
      <span class="sep">·</span>
      <span class="muted">非活动资金 <strong class="yellow">{fmtBalance(illiquidTotal())}</strong></span>
      <span class="sep">·</span>
      <span class="muted">总计 <strong class:neg={total() < 0} class:pos={total() >= 0}>{fmtBalance(total())}</strong></span>
    </div>
    <button class="primary" on:click={() => { showForm = !showForm; requestAnimationFrame(() => formEl?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })) }}>＋ 新建账户</button>
  </div>

  {#if error}<p class="neg" style="margin-bottom:8px">{error}</p>{/if}

  {#if showForm}
    <div class="card form-card" bind:this={formEl}>
      <div class="form-grid" style="grid-template-columns:1fr 1fr">
        <div class="form-row">
          <label for="acc-name">账户名称</label>
          <input id="acc-name" bind:value={form.name} placeholder="如：招商银行储蓄卡" />
        </div>
        <div class="form-row">
          <label for="acc-type">账户类型</label>
          <select id="acc-type" bind:value={form.account_type}>
            {#each ACCOUNT_TYPES as t}
              <option value={t.value}>{t.label}</option>
            {/each}
          </select>
        </div>
        <div class="form-row">
          <label for="acc-balance">初始余额（¥）</label>
          <input id="acc-balance" type="number" step="0.01" bind:value={form.initial_balance} />
        </div>
        <div class="form-row">
          <label for="acc-currency">货币</label>
          <select id="acc-currency" bind:value={form.currency}>
            <option value="CNY">CNY 人民币</option>
            <option value="USD">USD 美元</option>
          </select>
        </div>
      </div>
      <div class="liquid-toggle">
        <label class="toggle-label">
          <input type="checkbox" bind:checked={form.is_liquid} />
          <span>活动资金</span>
          <span class="toggle-hint muted">（取消勾选 = 非活动资金，如公积金、社保、限制提取账户）</span>
        </label>
      </div>
      <div class="actions">
        <button on:click={() => showForm = false}>取消</button>
        <button class="primary" disabled={submitting} on:click={submit}>
          {submitting ? '保存中…' : '保存'}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <div class="card" style="padding:20px">
      {#each Array(3) as _}
        <div class="skeleton-row">
          <div class="skeleton skeleton-bar" style="width:120px"></div>
          <div class="skeleton skeleton-bar" style="width:70px"></div>
          <div class="skeleton skeleton-bar" style="width:90px"></div>
          <div class="skeleton skeleton-bar" style="flex:1"></div>
        </div>
      {/each}
    </div>
  {:else if accounts.length === 0}
    <EmptyState
      icon="🏦"
      title="还没有账户"
      description="先创建你的银行卡、支付宝、微信等资金账户，然后就可以在「账本」中记录每月余额了。"
      action="＋ 创建第一个账户"
      onAction={() => { showForm = true; requestAnimationFrame(() => formEl?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })) }}
    />
  {:else}
    <div class="card">
      <table>
        <thead>
          <tr>
            <th class="sortable" on:click={() => toggleSort('name')}>账户名 {sortIcon('name')}</th>
            <th class="sortable" on:click={() => toggleSort('account_type')}>账户类型 {sortIcon('account_type')}</th>
            <th class="sortable" on:click={() => toggleSort('is_liquid')}>资金类型 {sortIcon('is_liquid')}</th>
            <th>货币</th>
            <th class="sortable" on:click={() => toggleSort('balance')}>余额 {sortIcon('balance')}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each sortedAccounts as acc}
            <tr>
              <td>{acc.name}</td>
              <td>{ACCOUNT_TYPE_LABELS[acc.account_type] ?? acc.account_type}</td>
              <td>
                <button
                  class="liquid-btn"
                  class:liquid={acc.is_liquid}
                  class:illiquid={!acc.is_liquid}
                  on:click={() => toggleLiquid(acc)}
                  title="点击切换"
                >
                  {acc.is_liquid ? '💧 活动资金' : '🔒 非活动资金'}
                </button>
              </td>
              <td>{acc.currency}</td>
              <td class:neg={acc.balance < 0} class:pos={acc.balance >= 0}>
                {fmtBalance(acc.balance)}
              </td>
              <td class="actions-cell">
                <button class="danger" on:click={() => del(acc.id, acc.name)}>删除</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
.toolbar { display: flex; justify-content: space-between; align-items: center; }
.sortable { cursor: pointer; user-select: none; }
.sortable:hover { color: var(--cyan); }
.totals { display: flex; align-items: center; gap: 8px; font-size: 13px; }
.sep { color: var(--border); }
.yellow { color: var(--yellow); }
.form-card { margin-bottom: 0; }
.liquid-toggle { margin-top: 12px; }
.toggle-label { display: flex; align-items: center; gap: 8px; font-size: 13px; cursor: pointer; }
.toggle-label input { width: auto; cursor: pointer; }
.toggle-hint { font-size: 12px; }

.liquid-btn {
  padding: 2px 10px;
  font-size: 12px;
  border-radius: 10px;
  cursor: pointer;
  border: 1px solid;
}
.liquid-btn.liquid { background: rgba(125,211,252,.12); color: var(--cyan); border-color: var(--cyan); }
.liquid-btn.illiquid { background: rgba(251,191,36,.12); color: var(--yellow); border-color: var(--yellow); }
.liquid-btn:hover { opacity: 0.75; }
.actions-cell { opacity: 0; transition: opacity 0.15s; }
tr:hover .actions-cell { opacity: 1; }
</style>
