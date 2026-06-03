import { writable } from 'svelte/store'
import { listAccounts, type Account } from './api'

export const accountsStore = writable<Account[]>([])
export const isDirtyStore = writable<boolean>(false)

export async function fetchAccounts() {
  try {
    const data = await listAccounts()
    accountsStore.set(data)
    return data
  } catch (e) {
    console.error('Failed to fetch accounts', e)
    throw e
  }
}
