<script lang="ts">
  import { onMount } from 'svelte'
  import { monthlyStats, categoryStats, fmtBalance, type MonthlyStats, type CategoryStat } from '../lib/api'
  import { Chart, ArcElement, Tooltip, Legend, DoughnutController } from 'chart.js'

  Chart.register(ArcElement, Tooltip, Legend, DoughnutController)

  const now = new Date()
  let year = now.getFullYear()
  let month = now.getMonth() + 1

  let stats: MonthlyStats | null = null
  let catStats: CategoryStat[] = []
  let loading = true
  let error = ''
  let chartCanvas: HTMLCanvasElement
  let chart: Chart | null = null

  async function load() {
    loading = true; error = ''
    try {
      const start = `${year}-${String(month).padStart(2,'0')}-01`
      const end = `${year}-${String(month).padStart(2,'0')}-31`
      ;[stats, catStats] = await Promise.all([
        monthlyStats(year, month),
        categoryStats(start, end),
      ])
      renderChart()
    } catch (e: any) {
      error = e.message
    } finally {
      loading = false
    }
  }

  function renderChart() {
    if (!chartCanvas || catStats.length === 0) return
    chart?.destroy()
    const COLORS = ['#7dd3fc','#4ade80','#fbbf24','#f87171','#a78bfa','#fb923c','#34d399','#e879f9']
    chart = new Chart(chartCanvas, {
      type: 'doughnut',
      data: {
        labels: catStats.map(c => c.category),
        datasets: [{ data: catStats.map(c => c.amount), backgroundColor: COLORS, borderWidth: 0 }],
      },
      options: {
        plugins: { legend: { position: 'right', labels: { color: '#7c7f9e', font: { size: 12 } } } },
        cutout: '65%',
      },
    })
  }

  function prevMonth() {
    if (month === 1) { year--; month = 12 } else month--
    load()
  }
  function nextMonth() {
    if (month === 12) { year++; month = 1 } else month++
    load()
  }
  const isCurrentMonth = () => year === now.getFullYear() && month === now.getMonth() + 1

  onMount(load)
</script>

<div class="overview">
  <div class="month-nav">
    <button on:click={prevMonth}>‹</button>
    <span>{year} 年 {month} 月</span>
    <button on:click={nextMonth} disabled={isCurrentMonth()}>›</button>
  </div>

  {#if loading}
    <p class="empty">加载中…</p>
  {:else if error}
    <p class="empty neg">{error}</p>
  {:else if stats}
    <div class="stat-grid">
      <div class="card stat">
        <div class="label">总资产</div>
        <div class="val" class:neg={stats.total_assets < 0}>{fmtBalance(stats.total_assets)}</div>
      </div>
      <div class="card stat">
        <div class="label">本月收入</div>
        <div class="val pos">{fmtBalance(stats.income)}</div>
      </div>
      <div class="card stat">
        <div class="label">本月支出</div>
        <div class="val neg">{fmtBalance(stats.expense)}</div>
      </div>
      <div class="card stat">
        <div class="label">本月结余</div>
        <div class="val" class:pos={stats.net >= 0} class:neg={stats.net < 0}>{fmtBalance(stats.net)}</div>
      </div>
    </div>

    {#if catStats.length > 0}
      <div class="card chart-card">
        <div class="card-title">本月支出分类</div>
        <div class="chart-wrap">
          <canvas bind:this={chartCanvas}></canvas>
        </div>
      </div>
    {/if}
  {/if}
</div>

<style>
.overview { display: flex; flex-direction: column; gap: 16px; }
.month-nav { display: flex; align-items: center; gap: 12px; font-size: 16px; font-weight: 600; }
.month-nav button { padding: 4px 12px; font-size: 18px; }
.stat-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 12px; }
.stat .label { font-size: 12px; color: var(--muted); margin-bottom: 6px; }
.stat .val { font-size: 22px; font-weight: 700; font-variant-numeric: tabular-nums; }
.chart-card .card-title { font-size: 13px; color: var(--muted); margin-bottom: 12px; }
.chart-wrap { max-width: 420px; margin: 0 auto; }
</style>
