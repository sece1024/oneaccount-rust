import { writable } from 'svelte/store'

type ConfirmState = {
  open: boolean
  title: string
  message: string
  confirmText: string
  danger: boolean
  resolve: ((v: boolean) => void) | null
}

export const confirmState = writable<ConfirmState>({
  open: false, title: '', message: '', confirmText: '确认', danger: true, resolve: null,
})

export function confirm(opts: { title?: string; message: string; confirmText?: string; danger?: boolean }): Promise<boolean> {
  return new Promise(resolve => {
    confirmState.set({
      open: true,
      title: opts.title ?? '确认操作',
      message: opts.message,
      confirmText: opts.confirmText ?? '确认',
      danger: opts.danger ?? true,
      resolve,
    })
  })
}
