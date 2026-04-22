<script lang="ts">
  import { onMount } from 'svelte'
  import { triggerBackup, listBackupFiles, type BackupFile } from '../lib/api'
  import { toast } from '../lib/toast'

  let backups: BackupFile[] = []
  let loading = false
  let creating = false

  async function loadBackups() {
    loading = true
    try { backups = await listBackupFiles() } catch (e: any) { toast.error(e.message) }
    loading = false
  }

  async function handleBackup() {
    creating = true
    try {
      const res = await triggerBackup()
      toast.success(`备份成功: ${res.file}`)
      await loadBackups()
    } catch (e: any) { toast.error(`备份失败: ${e.message}`) }
    creating = false
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`
  }

  onMount(loadBackups)
</script>

<div class="page">
  <div class="page-header">
    <h2>⚙️ 设置</h2>
  </div>

  <section class="card">
    <h3>📦 数据备份</h3>
    <p class="muted">备份文件保存在数据库同级目录下的 backups 文件夹中，服务启动时自动备份（保留最近 10 份）。</p>
    <button class="btn primary" on:click={handleBackup} disabled={creating}>
      {creating ? '备份中…' : '🗄️ 立即备份'}
    </button>
  </section>

  <section class="card">
    <h3>📋 备份历史</h3>
    {#if loading}
      <div class="skeleton" style="height:80px;border-radius:6px"></div>
    {:else if backups.length === 0}
      <p class="muted">暂无备份文件</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>文件名</th>
            <th style="text-align:right">大小</th>
          </tr>
        </thead>
        <tbody>
          {#each backups as b}
            <tr>
              <td>{b.name}</td>
              <td style="text-align:right">{fmtSize(b.size)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </section>

  <section class="card">
    <h3>ℹ️ 命令行工具</h3>
    <p class="muted">也可以通过命令行管理备份：</p>
    <pre class="cli-help">oneaccount backup            # 创建备份
oneaccount backups           # 列出所有备份
oneaccount restore &lt;文件名&gt;  # 恢复备份</pre>
  </section>
</div>

<style>
  .page { max-width: 720px; margin: 0 auto; padding: 24px; }
  .page-header { margin-bottom: 24px; }
  .page-header h2 { margin: 0; }
  .card {
    background: var(--bg2);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 20px;
    margin-bottom: 16px;
  }
  .card h3 { margin: 0 0 8px; font-size: 15px; }
  .card .muted { color: var(--muted); font-size: 13px; margin: 0 0 12px; }
  table { width: 100%; border-collapse: collapse; }
  th, td { padding: 8px 12px; text-align: left; border-bottom: 1px solid var(--border); }
  th { font-size: 11px; text-transform: uppercase; letter-spacing: .5px; color: var(--muted); }
  .cli-help {
    background: var(--bg);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 12px 16px;
    font-size: 13px;
    color: var(--muted);
    white-space: pre-wrap;
    margin: 0;
  }
</style>
