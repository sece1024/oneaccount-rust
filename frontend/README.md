# OneAccount Frontend

Frontend for OneAccount, built with Svelte 5 + TypeScript + Vite.

## Requirements

- Node.js 20+
- pnpm 9+

## Package Manager

This frontend uses pnpm.

- Keep `pnpm-lock.yaml` in version control.
- Do not keep `package-lock.json` at the same time.

## Install

```bash
cd frontend
pnpm install
```

## Development

```bash
pnpm run dev
```

Vite dev server will print the local URL in terminal.

## Type Check

```bash
pnpm run check
```

## Build

```bash
pnpm run build
```

Build output is written to `frontend/dist`.

## Preview Production Build

```bash
pnpm run preview
```

## Backend Integration

The Rust server serves static files from `frontend/dist`.

Typical workflow:

1. Build frontend with `pnpm run build`.
2. Start backend server from repository root (for example `make run-server`).

If `frontend/dist` is missing, backend API still starts, but static frontend pages are not available.
