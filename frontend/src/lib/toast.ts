import { writable } from 'svelte/store'

export interface ToastItem {
  id: number
  message: string
  type: 'success' | 'error' | 'info'
}

let nextId = 0

function createToastStore() {
  const { subscribe, update } = writable<ToastItem[]>([])

  function push(message: string, type: ToastItem['type'] = 'info', duration = 3000) {
    const id = nextId++
    update(items => [...items, { id, message, type }])
    setTimeout(() => dismiss(id), duration)
  }

  function dismiss(id: number) {
    update(items => items.filter(t => t.id !== id))
  }

  return {
    subscribe,
    success: (msg: string) => push(msg, 'success'),
    error: (msg: string) => push(msg, 'error', 5000),
    info: (msg: string) => push(msg, 'info'),
    dismiss,
  }
}

export const toast = createToastStore()
