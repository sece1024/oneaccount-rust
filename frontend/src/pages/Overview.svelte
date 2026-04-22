<script lang="ts">
  import { onMount, tick } from 'svelte'
  import {
    monthlyStats, categoryStats, getSnapshotGrid, fmtBalance,
    type MonthlyStats, type CategoryStat, type SnapshotGridRow,
  } from '../lib/api'
  import EmptyState from '../components/EmptyState.svelte'
  import {
    Chart, ArcElement, Tooltip, Legend, DoughnutController,
    LineElement, PointElement, CategoryScale, LinearScale, LineController, Filler,
  } from 'chart.js'

  Chart.register(
    ArcElement, Tooltip, Legend, DoughnutController,
    LineElement, PointElement, CategoryScale, LinearScale, LineController, Filler,
  )

  const now = new Date()
  let year = now.getFullYear()
  let month = now.getMonth() + 1

  let stats: MonthlyStats | null = null
  let catStats: CategoryStat[] = []
  let trendRows: SnapshotGridRow[] = []
  let loading = true
  let error = ''
  let chartCanvas: HTMLCanvasElement
  let chart: Chart | null = null
  let trendCanvas: HTMLCanvasElement
  let trendChart: Chart | null = null

  async function load() {
    loading = true; error = ''
    try {
      const start = `${year}-${String(month).padStart(2,'0')}-01`
      const end = `${year}-${String(month).padStart(2,'0')}-31`
      ;[stats, catStats, trendRows] = await Promise.all([
        monthlyStats(year, month),
        categoryStats(start, end),
        getSnapshotGrid(12),
      ])
    } catch (e: any) {
      error = e.message
    } finally {
      loading = false
    }
    await tick()
    renderChart()
    renderTrend()
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

  function renderTrend() {
    if (!trendCanvas || trendRows.length === 0) return
    trendChart?.destroy()
    const sorted = [...trendRows].sort((a, b) => a.year - b.year || a.month - b.month)
    const labels = sorted.map(r => `${r.year}-${String(r.month).padStart(2,'0')}`)
    trendChart = new Chart(trendCanvas, {
      type: 'line',
      data: {
        labels,
        datasets: [
          {
            label: '总资产',
            data: sorted.map(r => r.total),
            borderColor: '#4ade80',
            backgroundColor: 'rgba(74,222,128,.1)',
            fill: true,
            tension: 0.3,
            pointRadius: 3,
          },
          {
            label: '活动资金',
            data: sorted.map(r => r.liquid_total),
            borderColor: '#7dd3fc',
            tension: 0.3,
            pointRadius: 3,
          },
          {
            label: '非活动资金',
            data: sorted.map(r => r.illiquid_total),
            borderColor: '#fbbf24',
            tension: 0.3,
            pointRadius: 3,
          },
        ],
      },
      options: {
        responsive: true,
        interaction: { mode: 'index', intersect: false },
        scales: {
          x: { ticks: { color: '#7c7f9e', font: { size: 11 } }, grid: { color: 'rgba(255,255,255,.06)' } },
          y: { ticks: { color: '#7c7f9e', font: { size: 11 } }, grid: { color: 'rgba(255,255,255,.06)' } },
        },
        plugins: {
          legend: { labels: { color: '#7c7f9e', font: { size: 12 } } },
          tooltip: {
            callbacks: {
              label: (ctx: any) => `${ctx.dataset.label}: ¥${ctx.parsed.y?.toFixed(2) ?? '0.00'}`,
            },
          },
        },
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
    <div class="stat-grid">
      {#each Array(4) as _}
        <div class="card stat">
          <div class="skeleton skeleton-bar" style="width:60px;height:12px;margin-bottom:8px"></div>
          <div class="skeleton skeleton-bar h-lg" style="width:120px"></div>
        </div>
      {/each}
    </div>
  {:else if error}
    <p class="empty neg">{error}</p>
  {:else if stats}
    {#if stats.total_assets === 0 && stats.income === 0 && stats.expense === 0}
      <EmptyState
        icon="📊"
        title="暂无统计数据"
        description="在「账本」页面填写每月余额，或在「账目」页面记录收支，这里会自动生成统计。"
      />
    {:else}
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

    {#if trendRows.length > 0}
      <div class="card chart-card">
        <div class="card-title">资产趋势（近12个月）</div>
        <div class="trend-wrap">
          <canvas bind:this={trendCanvas}></canvas>
        </div>
      </div>
    {/if}
    {/if}
  {/if}
</div>

<style>
.overview { display: flex; flex-direction: column; gap: 16px; }
.month-nav { display: flex; align-items: center; gap: 12px; font-size: 16px; font-weight: 600; }
.month-nav button { padding: 4px 12px; font-size: 18px; }
.stat-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(160px, 1fr)); gap: 12px; }
.stat { transition: transform 0.2s, box-shadow 0.2s; }
.stat:hover { transform: translateY(-2px); box-shadow: var(--shadow-md); }
.stat .label { font-size: 12px; color: var(--muted); margin-bottom: 6px; }
.stat .val { font-size: 22px; font-weight: 700; font-variant-numeric: tabular-nums; }
.chart-card .card-title { font-size: 13px; color: var(--muted); margin-bottom: 12px; }
.chart-wrap { max-width: 420px; margin: 0 auto; }
.trend-wrap { max-width: 700px; }
</style>
