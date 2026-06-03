<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { toast } from '../lib/toast'
  import EmptyState from '../components/EmptyState.svelte'
  import {
    getSnapshotGrid, getEntryItems, saveSnapshot,
    fmtBalance,
    type SnapshotGridRow, type EntryItem, type Account,
  } from '../lib/api'
  import { accountsStore, fetchAccounts } from '../lib/stores'

  // ── 数据 ──────────────────────────────────────────────────────────────────

  $: accounts = $accountsStore
  let entryItems: EntryItem[] = []
  let historyRows: SnapshotGridRow[] = []

  let loading = true
  let error = ''

  // 编辑行：当前正在编辑的月份
  let editYear = new Date().getFullYear()
  let editMonth = new Date().getMonth() + 1
  // 每个账户的输入值（account_id -> string）
  let editValues: Record<number, string> = {}

  async function load() {
    loading = true; error = ''
    try {
      ;[entryItems, historyRows] = await Promise.all([
        getEntryItems(),
        getSnapshotGrid(24),
        fetchAccounts(),
      ])
      initEditRow()
    } catch (e: any) {
      error = e.message
    } finally {
      loading = false
    }
  }

  function initEditRow() {
    editValues = {}
    for (const item of entryItems) {
      editValues[item.account_id] = item.last_balance != null ? String(item.last_balance) : ''
    }
    const existing = historyRows.find(r => r.year === editYear && r.month === editMonth)
    if (existing) {
      for (const item of entryItems) {
        const v = existing.balances[String(item.account_id)]
        if (v != null) editValues[item.account_id] = String(v)
      }
    }
  }

  function roundToCents(n: number): number {
    return Math.round(n * 100) / 100
  }

  // ── 实时汇总（活动 / 非活动 / 总计） ─────────────────────────────────────

  function editTotals(): { liquid: number; illiquid: number; total: number } {
    let liquid = 0, illiquid = 0
    for (const item of entryItems) {
      const v = parseFloat(editValues[item.account_id] ?? '')
      if (isNaN(v)) continue
      const acc = accounts.find(a => a.id === item.account_id)
      if (acc?.is_liquid ?? true) liquid += v
      else illiquid += v
    }
    return { liquid: roundToCents(liquid), illiquid: roundToCents(illiquid), total: roundToCents(liquid + illiquid) }
  }

  // ── 保存 ─────────────────────────────────────────────────────────────────

  async function save() {
    const balances = entryItems
      .map(item => ({ account_id: item.account_id, balance: parseFloat(editValues[item.account_id] ?? '') }))
      .filter(b => !isNaN(b.balance))
    if (balances.length === 0) { error = '请至少填写一个账户余额'; return }
    error = ''
    try {
      await saveSnapshot(editYear, editMonth, balances)
      toast.success(`${editYear}-${String(editMonth).padStart(2,'0')} 已保存`)
      await load()
    } catch (e: any) {
      error = e.message
      toast.error(e.message)
    }
  }

  // ── 键盘导航 ─────────────────────────────────────────────────────────────

  async function handleKeydown(e: KeyboardEvent, idx: number) {
    if (e.key === 'Tab') {
      e.preventDefault()
      if (idx < entryItems.length - 1) {
        await tick(); focusCell(idx + 1)
      } else {
        await save()
      }
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (idx < entryItems.length - 1) {
        await tick(); focusCell(idx + 1)
      } else {
        await save()
      }
    }
  }

  function focusCell(idx: number) {
    const el = document.querySelector<HTMLInputElement>(`[data-cell="${idx}"]`)
    el?.focus(); el?.select()
  }

  // ── 月份导航 ─────────────────────────────────────────────────────────────

  function goPrevMonth() {
    if (editMonth === 1) { editYear--; editMonth = 12 } else editMonth--
    initEditRow()
  }
  function goNextMonth() {
    const now = new Date()
    if (editYear > now.getFullYear() || (editYear === now.getFullYear() && editMonth >= now.getMonth() + 1)) return
    if (editMonth === 12) { editYear++; editMonth = 1 } else editMonth++
    initEditRow()
  }
  const isCurrentOrFuture = () => {
    const now = new Date()
    return editYear > now.getFullYear() || (editYear === now.getFullYear() && editMonth >= now.getMonth() + 1)
  }

  // ── 点击历史行进入编辑 ────────────────────────────────────────────────────

  function editHistoryRow(row: SnapshotGridRow) {
    editYear = row.year
    editMonth = row.month
    editValues = {}
    for (const item of entryItems) {
      const v = row.balances[String(item.account_id)]
      editValues[item.account_id] = v != null ? String(v) : ''
    }
  }

  onMount(load)
</script>

<div class="ledger">

  <div class="toolbar">
    <div class="month-nav">
      <button on:click={goPrevMonth}>‹</button>
      <span class="month-label">编辑：{editYear}-{String(editMonth).padStart(2,'0')}</span>
      <button on:click={goNextMonth} disabled={isCurrentOrFuture()}>›</button>
    </div>
    <div class="right">
      {#if error}<span class="neg">{error}</span>{/if}
      <button class="primary" on:click={save}>保存本月</button>
    </div>
  </div>

  {#if loading}
    <div class="card" style="padding:20px">
      {#each Array(4) as _}
        <div class="skeleton-row">
          <div class="skeleton skeleton-bar" style="width:80px"></div>
          {#each Array(3) as _}
            <div class="skeleton skeleton-bar" style="flex:1"></div>
          {/each}
          <div class="skeleton skeleton-bar" style="width:100px"></div>
        </div>
      {/each}
    </div>
  {:else if accounts.length === 0}
    <EmptyState
      icon="📒"
      title="还没有账户"
      description="请先到「账户管理」页面创建账户，然后就可以在这里像填表格一样记录每月余额了。"
    />
  {:else}
    <div class="table-wrap">
      <table class="ledger-table">
        <thead>
          <tr>
            <th class="col-month">月份</th>
            {#each entryItems as item}
              {@const acc = accounts.find(a => a.id === item.account_id)}
              <th class="col-account" class:col-illiquid={acc && !acc.is_liquid}>
                <div class="acc-name">{item.account_name}</div>
                <div class="acc-type">{item.account_type}</div>
              </th>
            {/each}
            <th class="col-total col-liquid">活动资金</th>
            <th class="col-total col-restricted">非活动资金</th>
            <th class="col-total col-grand">总资产</th>
          </tr>
        </thead>
        <tbody>

          <!-- 编辑行 -->
          <tr class="edit-row">
            <td class="col-month edit-month">
              {editYear}-{String(editMonth).padStart(2,'0')}
              <span class="badge">编辑中</span>
            </td>
            {#each entryItems as item, idx}
              {@const acc = accounts.find(a => a.id === item.account_id)}
              <td class="col-account" class:col-illiquid={acc && !acc.is_liquid}>
                <input
                  class="cell-input"
                  class:neg-input={parseFloat(editValues[item.account_id] ?? '') < 0}
                  data-cell={idx}
                  type="number"
                  step="0.01"
                  bind:value={editValues[item.account_id]}
                  placeholder="—"
                  on:keydown={e => handleKeydown(e, idx)}
                />
              </td>
            {/each}
            <td class="col-total" class:pos={editTotals().liquid >= 0} class:neg={editTotals().liquid < 0}>{fmtBalance(editTotals().liquid)}</td>
            <td class="col-total col-restricted-val" class:pos={editTotals().illiquid >= 0} class:neg={editTotals().illiquid < 0}>{fmtBalance(editTotals().illiquid)}</td>
            <td class="col-total col-grand-val" class:pos={editTotals().total >= 0} class:neg={editTotals().total < 0}>{fmtBalance(editTotals().total)}</td>
          </tr>

          <!-- 历史行（最新在上） -->
          {#each [...historyRows].reverse() as row}
            {@const isEditing = row.year === editYear && row.month === editMonth}
            {#if !isEditing}
              <tr class="history-row" on:click={() => editHistoryRow(row)} title="点击重新编辑">
                <td class="col-month">{row.year}-{String(row.month).padStart(2,'0')}</td>
                {#each entryItems as item}
                  {@const bal = row.balances[String(item.account_id)]}
                  {@const acc = accounts.find(a => a.id === item.account_id)}
                  <td class="col-account" class:neg={bal != null && bal < 0} class:pos={bal != null && bal >= 0} class:col-illiquid={acc && !acc.is_liquid}>
                    {#if bal != null}{fmtBalance(bal)}{:else}<span class="muted">—</span>{/if}
                  </td>
                {/each}
                <td class="col-total" class:pos={row.liquid_total >= 0} class:neg={row.liquid_total < 0}>{fmtBalance(row.liquid_total)}</td>
                <td class="col-total col-restricted-val" class:pos={row.illiquid_total >= 0} class:neg={row.illiquid_total < 0}>{fmtBalance(row.illiquid_total)}</td>
                <td class="col-total col-grand-val" class:pos={row.total >= 0} class:neg={row.total < 0}>{fmtBalance(row.total)}</td>
              </tr>
            {/if}
          {/each}

        </tbody>
      </table>
    </div>

    <div class="legend">
      <span class="dot liquid"></span> 活动资金（可自由支配）
      <span class="dot illiquid"></span> 非活动资金（限制提取，如公积金、社保）
    </div>

    <p class="hint muted">Tab / Enter 切换账户 · 最后一列 Tab 或点击「保存本月」提交 · 点击历史行可重新编辑 · 在账户管理中设置资金类型</p>
  {/if}
</div>

<style>
.ledger { display: flex; flex-direction: column; gap: 12px; }

.toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
}
.month-nav { display: flex; align-items: center; gap: 8px; }
.month-nav button { padding: 4px 12px; font-size: 16px; }
.month-label { font-size: 15px; font-weight: 600; min-width: 120px; text-align: center; }
.right { display: flex; align-items: center; gap: 10px; }

.table-wrap { overflow-x: auto; max-height: 70vh; overflow-y: auto; }

.ledger-table {
  width: 100%;
  border-collapse: collapse;
  background: var(--bg2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  overflow: hidden;
}

th {
  background: var(--bg3);
  padding: 10px 14px;
  font-size: 12px;
  color: var(--muted);
  white-space: nowrap;
  position: sticky;
  top: 0;
  z-index: 2;
}
th.col-month { min-width: 90px; }
th.col-account { min-width: 110px; text-align: right; }
th.col-illiquid { background: rgba(251,191,36,.06); }
th.col-total { min-width: 110px; text-align: right; border-left: 1px solid var(--border); }
th.col-liquid { color: var(--cyan); }
th.col-restricted { color: var(--yellow); }
th.col-grand { color: var(--green); }

.acc-name { color: var(--text); font-size: 12px; }
.acc-type { color: var(--muted); font-size: 11px; margin-top: 2px; }

td { padding: 8px 14px; border-bottom: 1px solid var(--border); }
td.col-account, td.col-total { text-align: right; font-variant-numeric: tabular-nums; }
td.col-total { font-weight: 600; border-left: 1px solid var(--border); }
td.col-month { color: var(--muted); font-size: 13px; white-space: nowrap; }
td.col-illiquid { background: rgba(251,191,36,.04); }
td.col-restricted-val { color: var(--yellow); }
td.col-grand-val { color: var(--text); }

/* 编辑行 — sticky */
.edit-row { background: rgba(125, 211, 252, 0.05); }
.edit-row td {
  border-bottom: 2px solid var(--cyan) !important;
  position: sticky;
  top: 42px; /* below thead */
  z-index: 1;
  background: inherit;
}
.edit-row { background: var(--bg2); }
.edit-month { color: var(--cyan) !important; font-weight: 600; }
.badge {
  display: inline-block;
  margin-left: 6px;
  font-size: 10px;
  background: var(--cyan);
  color: #0f1117;
  border-radius: 3px;
  padding: 1px 5px;
  vertical-align: middle;
}

.cell-input {
  width: 100%;
  text-align: right;
  background: var(--bg);
  border: 1px solid var(--border);
  border-radius: 4px;
  padding: 4px 8px;
  font-size: 13px;
  font-variant-numeric: tabular-nums;
  color: var(--green);
}
.cell-input.neg-input { color: var(--red); }
.cell-input:focus { border-color: var(--cyan); outline: none; }
.cell-input::placeholder { color: var(--border); }

/* 历史行 */
.history-row { cursor: pointer; transition: background 0.1s; }
.history-row:hover td { background: var(--bg3); }
.history-row:last-child td { border-bottom: none; }

/* 图例 */
.legend { display: flex; align-items: center; gap: 14px; font-size: 12px; color: var(--muted); }
.dot { display: inline-block; width: 10px; height: 10px; border-radius: 50%; }
.dot.liquid { background: var(--cyan); }
.dot.illiquid { background: var(--yellow); }

.hint { font-size: 12px; text-align: center; padding: 4px 0; }
</style>
