import {
  createContext,
  type PropsWithChildren,
  useContext,
  useMemo,
  useState,
} from "react";

export const BACKEND_CONFIG_STORAGE_KEY = "labInventory.apiBaseUrl";
const DEFAULT_API_BASE_URL =
  import.meta.env.VITE_DEFAULT_API_BASE_URL ?? "http://127.0.0.1:8000/api/v1";

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
  const defaultApiBaseUrl = normalizeApiBaseUrl(DEFAULT_API_BASE_URL);
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
