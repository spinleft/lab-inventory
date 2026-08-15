import {
  createContext,
  type PropsWithChildren,
  useContext,
  useMemo,
  useState,
} from "react";

export const BACKEND_CONFIG_STORAGE_KEY = "labInventory.apiBaseUrl";

declare global {
  interface Window {
    __LAB_INVENTORY_CONFIG__?: { apiBaseUrl?: string };
  }
}

/**
 * The API the app talks to when the user has not chosen one.
 *
 * Three layers, most specific first. `config.js` is written by the container at
 * start-up, so one built image serves any deployment; the Vite variable is
 * baked in at build time, which is what the Tauri bundles use; the literal is
 * the local development backend.
 */
export function resolveDefaultApiBaseUrl() {
  const runtime = window.__LAB_INVENTORY_CONFIG__?.apiBaseUrl?.trim();
  const buildTime = import.meta.env.VITE_DEFAULT_API_BASE_URL?.trim();
  const configured = runtime || buildTime || "http://127.0.0.1:8000/api/v1";

  // A deployment that serves the app and the API from one origin configures
  // `/api/v1`, which only means anything relative to where the page came from.
  return configured.startsWith("/")
    ? `${window.location.origin}${configured}`
    : configured;
}

type BackendConfigContextValue = {
  apiBaseUrl: string;
  defaultApiBaseUrl: string;
  hasConfiguredApiBaseUrl: boolean;
  resetApiBaseUrl: () => string;
  setApiBaseUrl: (input: string) => string;
};

const BackendConfigContext = createContext<BackendConfigContextValue | null>(null);

export class BackendConfigError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackendConfigError";
  }
}

export function BackendConfigProvider({ children }: PropsWithChildren) {
  const defaultApiBaseUrl = normalizeApiBaseUrl(resolveDefaultApiBaseUrl());
  const initialConfig = readStoredApiBaseUrl(defaultApiBaseUrl);
  const [apiBaseUrl, setApiBaseUrlState] = useState(initialConfig.apiBaseUrl);
  const [hasConfiguredApiBaseUrl, setHasConfiguredApiBaseUrl] = useState(
    initialConfig.hasConfiguredApiBaseUrl,
  );

  const value = useMemo<BackendConfigContextValue>(
    () => ({
      apiBaseUrl,
      defaultApiBaseUrl,
      hasConfiguredApiBaseUrl,
      resetApiBaseUrl: () => {
        window.localStorage.removeItem(BACKEND_CONFIG_STORAGE_KEY);
        setApiBaseUrlState(defaultApiBaseUrl);
        setHasConfiguredApiBaseUrl(false);
        return defaultApiBaseUrl;
      },
      setApiBaseUrl: (input: string) => {
        const normalized = normalizeApiBaseUrl(input);
        window.localStorage.setItem(BACKEND_CONFIG_STORAGE_KEY, normalized);
        setApiBaseUrlState(normalized);
        setHasConfiguredApiBaseUrl(true);
        return normalized;
      },
    }),
    [apiBaseUrl, defaultApiBaseUrl, hasConfiguredApiBaseUrl],
  );

  return (
    <BackendConfigContext.Provider value={value}>
      {children}
    </BackendConfigContext.Provider>
  );
}

export function useBackendConfig() {
  const context = useContext(BackendConfigContext);
  if (!context) {
    throw new Error("useBackendConfig must be used inside BackendConfigProvider.");
  }
  return context;
}

function readStoredApiBaseUrl(defaultApiBaseUrl: string) {
  const stored = window.localStorage.getItem(BACKEND_CONFIG_STORAGE_KEY);
  if (!stored) {
    return {
      apiBaseUrl: defaultApiBaseUrl,
      hasConfiguredApiBaseUrl: false,
    };
  }

  try {
    return {
      apiBaseUrl: normalizeApiBaseUrl(stored),
      hasConfiguredApiBaseUrl: true,
    };
  } catch {
    window.localStorage.removeItem(BACKEND_CONFIG_STORAGE_KEY);
    return {
      apiBaseUrl: defaultApiBaseUrl,
      hasConfiguredApiBaseUrl: false,
    };
  }
}

export function normalizeApiBaseUrl(input: string) {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new BackendConfigError("请输入后端 API 地址。");
  }

  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    throw new BackendConfigError("后端 API 地址必须是有效的 URL。");
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new BackendConfigError("后端 API 地址必须使用 http 或 https。");
  }

  url.hash = "";
  url.search = "";

  const normalizedPath = url.pathname.replace(/\/+$/, "");
  if (!normalizedPath || normalizedPath === "") {
    url.pathname = "/api/v1";
  } else if (normalizedPath.endsWith("/api/v1")) {
    url.pathname = normalizedPath;
  } else {
    url.pathname = `${normalizedPath}/api/v1`;
  }

  return url.toString().replace(/\/$/, "");
}
