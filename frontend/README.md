# Lab Inventory Frontend

React/Vite frontend shared by the web app and the Tauri desktop client.

## Scripts

- `npm run dev` starts the Vite web app on `127.0.0.1:5173`.
- `npm run test` runs Vitest and Testing Library tests.
- `npm run test:e2e` runs the Playwright smoke tests across desktop, tablet, and mobile browser viewports.
- `npm run typecheck` checks TypeScript without emitting files.
- `npm run build` typechecks and builds the web bundle.
- `npm run tauri:dev` opens the same app in the Tauri shell.
- `npm run tauri:build` builds the desktop package.
- `npm run tauri:android:init` initializes the Android project after the Android toolchain is installed.
- `npm run tauri:android:build` builds the Android package after initialization.
- `npm run tauri:ios:init` initializes the iOS project on macOS after the iOS toolchain is installed.
- `npm run tauri:ios:build` builds the iOS package after initialization.

The default backend API is `http://127.0.0.1:8000/api/v1`.
