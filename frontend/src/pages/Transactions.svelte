<script lang="ts">
  import { onMount } from 'svelte'
  import {
    listTransactions, createTransaction, deleteTransaction,
    listAccounts, listCategories,
    fmtBalance,
    type Transaction, type Account, type Category,
  } from '../lib/api'

  let txs: Transaction[] = []
  let accounts: Account[] = []
  let categories: Category[] = []
  let loading = true
  let error = ''
  let showForm = false
  let submitting = false

  // filters
  let filterType = ''
  let filterAccount = ''

  let form = {
    amount: '',
    transaction_type: 'expense' as 'income' | 'expense' | 'transfer',
    category_id: '' as any,
    account_id: '' as any,
    to_account_id: '' as any,
    date: new Date().toISOString().slice(0, 10),
    note: '',
    is_large: false,
  }

  async function load() {
    loading = true; error = ''
    try {
      ;[txs, accounts, categories] = await Promise.all([
        listTransactions({
          tx_type: filterType || undefined,
          account_id: filterAccount ? Number(filterAccount) : undefined,
          limit: 100,
        }),
        listAccounts(),
        listCategories(),
      ])
    } catch (e: any) { error = e.message }
    finally { loading = false }
  }

  async function submit() {
    const amount = parseFloat(form.amount)
    if (isNaN(amount) || amount <= 0) { error = '请输入有效金额'; return }
    if (!form.account_id) { error = '请选择账户'; return }
    submitting = true; error = ''
    try {
      await createTransaction({
        amount,
        transaction_type: form.transaction_type,
        category_id: form.category_id ? Number(form.category_id) : null,
        account_id: Number(form.account_id),
        to_account_id: form.to_account_id ? Number(form.to_account_id) : null,
        date: form.date,
        note: form.note || null,
        is_large: form.is_large,
      })
      showForm = false
      form = { amount: '', transaction_type: 'expense', category_id: '', account_id: '', to_account_id: '', date: new Date().toISOString().slice(0,10), note: '', is_large: false }
      await load()
    } catch (e: any) { error = e.message }
    finally { submitting = false }
  }

  async function del(id: number) {
    if (!confirm('确认删除这条账目？')) return
    try { await deleteTransaction(id); await load() }
    catch (e: any) { error = e.message }
  }

  const expenseCategories = () => categories.filter(c => c.category_type === 'expense')
  const incomeCategories = () => categories.filter(c => c.category_type === 'income')
  const visibleCategories = () => form.transaction_type === 'income' ? incomeCategories() : expenseCategories()

  function typeLabel(t: string) {
    return t === 'income' ? '收入' : t === 'expense' ? '支出' : '转账'
  }

  onMount(load)
</script>

<div class="transactions">
  <div class="toolbar">
    <div class="filters">
      <select bind:value={filterType} on:change={load}>
        <option value="">全部类型</option>
        <option value="income">收入</option>
        <option value="expense">支出</option>
        <option value="transfer">转账</option>
      </select>
      <select bind:value={filterAccount} on:change={load}>
        <option value="">全部账户</option>
        {#each accounts as acc}
          <option value={acc.id}>{acc.name}</option>
        {/each}
      </select>
    </div>
    <button class="primary" on:click={() => showForm = !showForm}>＋ 新增账目</button>
  </div>

  {#if error}<p class="neg" style="margin:8px 0">{error}</p>{/if}

  {#if showForm}
    <div class="card form-card">
      <div class="form-grid" style="grid-template-columns:1fr 1fr 1fr">
        <div class="form-row">
          <label for="tx-type">类型</label>
          <select id="tx-type" bind:value={form.transaction_type}>
            <option value="expense">支出</option>
            <option value="income">收入</option>
            <option value="transfer">转账</option>
          </select>
        </div>
        <div class="form-row">
          <label for="tx-amount">金额（¥）</label>
          <input id="tx-amount" type="number" step="0.01" min="0.01" bind:value={form.amount} placeholder="0.00" />
        </div>
        <div class="form-row">
          <label for="tx-date">日期</label>
          <input id="tx-date" type="date" bind:value={form.date} />
        </div>
        <div class="form-row">
          <label for="tx-account">账户</label>
          <select id="tx-account" bind:value={form.account_id}>
            <option value="">请选择</option>
            {#each accounts as acc}
              <option value={acc.id}>{acc.name}</option>
            {/each}
          </select>
        </div>
        {#if form.transaction_type === 'transfer'}
          <div class="form-row">
            <label for="tx-to-account">转入账户</label>
            <select id="tx-to-account" bind:value={form.to_account_id}>
              <option value="">请选择</option>
              {#each accounts as acc}
                <option value={acc.id}>{acc.name}</option>
              {/each}
            </select>
          </div>
        {:else}
          <div class="form-row">
            <label for="tx-category">分类</label>
            <select id="tx-category" bind:value={form.category_id}>
              <option value="">不分类</option>
              {#each visibleCategories() as cat}
                <option value={cat.id}>{cat.icon ?? ''} {cat.name}</option>
              {/each}
            </select>
          </div>
        {/if}
        <div class="form-row">
          <label for="tx-note">备注</label>
          <input id="tx-note" bind:value={form.note} placeholder="可选" />
        </div>
      </div>
      <div class="actions">
        <label class="large-check">
          <input type="checkbox" bind:checked={form.is_large} /> 标记大额
        </label>
        <button on:click={() => { showForm = false; error = '' }}>取消</button>
        <button class="primary" disabled={submitting} on:click={submit}>
          {submitting ? '提交中…' : '提交'}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <p class="empty">加载中…</p>
  {:else if txs.length === 0}
    <p class="empty">暂无账目</p>
  {:else}
    <div class="card">
      <table>
        <thead>
          <tr><th>日期</th><th>类型</th><th>分类</th><th>账户</th><th>金额</th><th>备注</th><th></th></tr>
        </thead>
        <tbody>
          {#each txs as tx}
            <tr>
              <td class="muted">{tx.date}</td>
              <td><span class="tag {tx.transaction_type}">{typeLabel(tx.transaction_type)}</span></td>
              <td class="muted">{tx.category_name ?? '—'}</td>
              <td>{tx.account_name ?? '—'}{tx.to_account_name ? ` → ${tx.to_account_name}` : ''}</td>
              <td class:neg={tx.transaction_type === 'expense'} class:pos={tx.transaction_type === 'income'}>
                {tx.transaction_type === 'expense' ? '-' : tx.transaction_type === 'income' ? '+' : ''}{fmtBalance(tx.amount)}
              </td>
              <td class="muted">{tx.note ?? ''}{tx.is_large ? ' 🔴' : ''}</td>
              <td><button class="danger" on:click={() => del(tx.id)}>删除</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</div>

<style>
.toolbar { display: flex; justify-content: space-between; align-items: center; gap: 8px; }
.filters { display: flex; gap: 8px; }
.filters select { width: auto; }
.form-card { margin-bottom: 0; }
.large-check { display: flex; align-items: center; gap: 6px; font-size: 13px; margin-right: auto; }
.large-check input { width: auto; }
</style>
