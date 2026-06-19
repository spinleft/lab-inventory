# Lab Inventory Frontend

The frontend is a React/Vite application shared by the browser client and the Tauri desktop shell.

The Web UI is intentionally separate from `src-tauri`. Tauri remains the platform wrapper; the React application owns routing, API calls, theme, interaction logic, and module registration.

## Scripts

- `npm run dev` starts Vite on `127.0.0.1:5173`.
- `npm run typecheck` runs TypeScript without emitting files.
- `npm run test` runs Vitest component and unit tests.
- `npm run test:e2e` runs Playwright smoke tests across desktop, tablet, and mobile browser viewports.
- `npm run build` typechecks and builds the Web bundle.
- `npm run tauri:dev` opens the same app in the Tauri shell.
- `npm run tauri:build` builds the desktop package.

The default backend API is `http://127.0.0.1:8000/api/v1`.
