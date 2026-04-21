const BASE = '/api/v1'

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(BASE + path, {
    headers: { 'Content-Type': 'application/json', ...init?.headers },
    ...init,
  })
  if (!res.ok) {
    const text = await res.text()
    throw new Error(text || `HTTP ${res.status}`)
  }
  if (res.status === 204) return undefined as T
  return res.json()
}

// ── Types ─────────────────────────────────────────────────────────────────────

export interface Account {
  id: number
  name: string
  account_type: string
  currency: string
  balance: number
  is_liquid: boolean
  created_at: string
  updated_at: string
}

export interface NewAccount {
  name: string
  account_type: string
  currency: string
  initial_balance: number
  is_liquid: boolean
}

export interface Category {
  id: number
  name: string
  category_type: 'income' | 'expense'
  icon: string | null
  parent_id: number | null
}

export interface Transaction {
  id: number
  amount: number
  transaction_type: 'income' | 'expense' | 'transfer'
  category_id: number | null
  category_name: string | null
  account_id: number
  account_name: string | null
  to_account_id: number | null
  to_account_name: string | null
  date: string
  note: string | null
  is_large: boolean
  created_at: string
  updated_at: string
}

export interface NewTransaction {
  amount: number
  transaction_type: 'income' | 'expense' | 'transfer'
  category_id: number | null
  account_id: number
  to_account_id: number | null
  date: string
  note: string | null
  is_large: boolean
}

export interface MonthlyStats {
  year: number
  month: number
  income: number
  expense: number
  net: number
  total_assets: number
}

export interface CategoryStat {
  category: string
  amount: number
}

export interface TxFilter {
  start_date?: string
  end_date?: string
  account_id?: number
  category_id?: number
  tx_type?: string
  limit?: number
  offset?: number
}

// ── Accounts ──────────────────────────────────────────────────────────────────

export const listAccounts = () => request<Account[]>('/accounts')

export const createAccount = (data: NewAccount) =>
  request<Account>('/accounts', { method: 'POST', body: JSON.stringify(data) })

export const deleteAccount = (id: number) =>
  request<void>(`/accounts/${id}`, { method: 'DELETE' })

export const updateAccountLiquid = (id: number, is_liquid: boolean) =>
  request<void>(`/accounts/${id}/liquid`, { method: 'POST', body: JSON.stringify({ is_liquid }) })

// ── Categories ────────────────────────────────────────────────────────────────

export const listCategories = () => request<Category[]>('/categories')

// ── Transactions ──────────────────────────────────────────────────────────────

export const listTransactions = (filter: TxFilter = {}) => {
  const params = new URLSearchParams()
  Object.entries(filter).forEach(([k, v]) => {
    if (v !== undefined && v !== null && v !== '') params.set(k, String(v))
  })
  return request<Transaction[]>(`/transactions?${params}`)
}

export const createTransaction = (data: NewTransaction) =>
  request<Transaction>('/transactions', { method: 'POST', body: JSON.stringify(data) })

export const updateTransaction = (id: number, data: NewTransaction) =>
  request<Transaction>(`/transactions/${id}`, { method: 'PUT', body: JSON.stringify(data) })

export const deleteTransaction = (id: number) =>
  request<void>(`/transactions/${id}`, { method: 'DELETE' })

// ── Stats ─────────────────────────────────────────────────────────────────────

export const monthlyStats = (year?: number, month?: number) => {
  const params = new URLSearchParams()
  if (year) params.set('year', String(year))
  if (month) params.set('month', String(month))
  return request<MonthlyStats>(`/stats/monthly?${params}`)
}

export const categoryStats = (start_date?: string, end_date?: string) => {
  const params = new URLSearchParams()
  if (start_date) params.set('start_date', start_date)
  if (end_date) params.set('end_date', end_date)
  return request<CategoryStat[]>(`/stats/by-category?${params}`)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

export function fmtBalance(b: number): string {
  const abs = Math.abs(b).toLocaleString('zh-CN', { minimumFractionDigits: 2, maximumFractionDigits: 2 })
  return b < 0 ? `-¥${abs}` : `¥${abs}`
}

export const ACCOUNT_TYPE_LABELS: Record<string, string> = {
  cash: '现金',
  bank: '银行卡',
  wechat: '微信',
  alipay: '支付宝',
  stock: '股票',
  crypto: '虚拟货币',
  social_insurance: '社保',
  fund: '基金',
  credit_card: '信用/负债',
}

export const ACCOUNT_TYPES = Object.entries(ACCOUNT_TYPE_LABELS).map(([value, label]) => ({ value, label }))

// ── Snapshots ─────────────────────────────────────────────────────────────────

export interface SnapshotGridRow {
  year: number
  month: number
  /** account_id (string key from JSON) -> balance */
  balances: Record<string, number>
  total: number
  liquid_total: number
  illiquid_total: number
}

export interface EntryItem {
  account_id: number
  account_name: string
  account_type: string
  last_balance: number | null
}

export const getSnapshotGrid = (months = 18) =>
  request<SnapshotGridRow[]>(`/snapshots/grid?months=${months}`)

export const getEntryItems = () =>
  request<EntryItem[]>('/snapshots/entry-items')

export const saveSnapshot = (year: number, month: number, balances: { account_id: number; balance: number }[], note?: string) =>
  request<{ total: number }>('/snapshots', {
    method: 'POST',
    body: JSON.stringify({ year, month, balances, note }),
  })
