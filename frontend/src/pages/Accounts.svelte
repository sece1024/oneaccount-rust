<script lang="ts">
  import { onMount } from 'svelte'
  import {
    listAccounts, createAccount, deleteAccount,
    fmtBalance, ACCOUNT_TYPES, ACCOUNT_TYPE_LABELS,
    type Account,
  } from '../lib/api'

  let accounts: Account[] = []
  let loading = true
  let error = ''
  let showForm = false
  let submitting = false

  let form = { name: '', account_type: 'bank', currency: 'CNY', initial_balance: 0 }

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
      form = { name: '', account_type: 'bank', currency: 'CNY', initial_balance: 0 }
      await load()
    } catch (e: any) { error = e.message }
    finally { submitting = false }
  }

  async function del(id: number, name: string) {
    if (!confirm(`确认删除账户「${name}」？`)) return
    try { await deleteAccount(id); await load() }
    catch (e: any) { error = e.message }
  }

  const total = () => accounts.reduce((s, a) => s + a.balance, 0)

  onMount(load)
</script>

<div class="accounts">
  <div class="toolbar">
    <span class="muted">共 {accounts.length} 个账户，总计
      <strong class:neg={total() < 0} class:pos={total() >= 0}>{fmtBalance(total())}</strong>
    </span>
    <button class="primary" on:click={() => showForm = !showForm}>＋ 新建账户</button>
  </div>

  {#if error}<p class="neg" style="margin-bottom:8px">{error}</p>{/if}

  {#if showForm}
    <div class="card form-card">
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
      <div class="actions">
        <button on:click={() => showForm = false}>取消</button>
        <button class="primary" disabled={submitting} on:click={submit}>
          {submitting ? '保存中…' : '保存'}
        </button>
      </div>
    </div>
  {/if}

  {#if loading}
    <p class="empty">加载中…</p>
  {:else if accounts.length === 0}
    <p class="empty">暂无账户</p>
  {:else}
    <div class="card">
      <table>
        <thead>
          <tr><th>账户名</th><th>类型</th><th>货币</th><th>余额</th><th></th></tr>
        </thead>
        <tbody>
          {#each accounts as acc}
            <tr>
              <td>{acc.name}</td>
              <td>{ACCOUNT_TYPE_LABELS[acc.account_type] ?? acc.account_type}</td>
              <td>{acc.currency}</td>
              <td class:neg={acc.balance < 0} class:pos={acc.balance >= 0}>
                {fmtBalance(acc.balance)}
              </td>
              <td>
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
.form-card { margin-bottom: 0; }
</style>
