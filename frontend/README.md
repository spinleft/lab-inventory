# Lab Inventory Frontend

Single React/Vite frontend for both the web app and the Tauri desktop client.

## Setup

```bash
npm install
npm run dev
```

The default backend API is configured by `VITE_DEFAULT_API_BASE_URL`.
At runtime, users can change the backend API URL from the login page before authentication.
The value is stored in `localStorage` for the current browser or Tauri WebView.

## Scripts

- `npm run dev` starts the Vite web app on `127.0.0.1:5173`.
- `npm run test` runs Vitest and Testing Library tests.
- `npm run test:e2e` runs Playwright browser tests.
- `npm run build` typechecks and builds the web bundle.
- `npm run tauri:dev` starts the desktop shell against the Vite dev server.
- `npm run tauri:build` builds the desktop package.

## Backend Notes

All API calls use `credentials: "include"` and target `/api/v1`.
When the frontend and backend run on different origins, the backend must include the frontend origin in `cors_allowed_origins`.
For cross-site web deployments, session cookies may also need `SameSite=None; Secure`.
