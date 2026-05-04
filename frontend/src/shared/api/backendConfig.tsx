import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useMemo,
  useState,
} from "react";

const STORAGE_KEY = "lab-inventory.backend-api-base-url";
const FALLBACK_API_BASE_URL = "http://127.0.0.1:8000/api/v1";

type BackendConfigContextValue = {
  apiBaseUrl: string;
  defaultApiBaseUrl: string;
  setApiBaseUrl: (value: string) => string;
  resetApiBaseUrl: () => void;
};

const BackendConfigContext = createContext<BackendConfigContextValue | null>(
  null,
);

export class BackendConfigError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "BackendConfigError";
  }
}

export function getDefaultApiBaseUrl() {
  return normalizeApiBaseUrl(
    import.meta.env.VITE_DEFAULT_API_BASE_URL ?? FALLBACK_API_BASE_URL,
  );
}

export function normalizeApiBaseUrl(input: string) {
  const value = input.trim();
  if (!value) {
    throw new BackendConfigError("请输入后端服务器地址。");
  }

  let url: URL;
  try {
    url = new URL(value);
  } catch {
    throw new BackendConfigError("后端服务器地址必须是完整的 URL。");
  }

  if (url.protocol !== "http:" && url.protocol !== "https:") {
    throw new BackendConfigError("后端服务器地址必须使用 http 或 https。");
  }

  url.hash = "";
  url.search = "";

  const normalizedPath = stripTrailingSlash(url.pathname);
  if (!normalizedPath || normalizedPath === "") {
    url.pathname = "/api/v1";
  } else if (normalizedPath.endsWith("/api/v1")) {
    url.pathname = normalizedPath;
  } else {
    url.pathname = `${normalizedPath}/api/v1`;
  }

  return stripTrailingSlash(url.toString());
}

export function readStoredApiBaseUrl() {
  const defaultValue = getDefaultApiBaseUrl();
  if (!storageAvailable()) {
    return defaultValue;
  }

  const storedValue = window.localStorage.getItem(STORAGE_KEY);
  if (!storedValue) {
    return defaultValue;
  }

  try {
    return normalizeApiBaseUrl(storedValue);
  } catch {
    window.localStorage.removeItem(STORAGE_KEY);
    return defaultValue;
  }
}

export function BackendConfigProvider({ children }: PropsWithChildren) {
  const defaultApiBaseUrl = useMemo(() => getDefaultApiBaseUrl(), []);
  const [apiBaseUrl, setApiBaseUrlState] = useState(() => readStoredApiBaseUrl());

  const setApiBaseUrl = useCallback((value: string) => {
    const normalized = normalizeApiBaseUrl(value);
    if (storageAvailable()) {
      window.localStorage.setItem(STORAGE_KEY, normalized);
    }
    setApiBaseUrlState(normalized);
    return normalized;
  }, []);

  const resetApiBaseUrl = useCallback(() => {
    if (storageAvailable()) {
      window.localStorage.removeItem(STORAGE_KEY);
    }
    setApiBaseUrlState(defaultApiBaseUrl);
  }, [defaultApiBaseUrl]);

  const contextValue = useMemo(
    () => ({ apiBaseUrl, defaultApiBaseUrl, setApiBaseUrl, resetApiBaseUrl }),
    [apiBaseUrl, defaultApiBaseUrl, resetApiBaseUrl, setApiBaseUrl],
  );

  return (
    <BackendConfigContext.Provider value={contextValue}>
      {children}
    </BackendConfigContext.Provider>
  );
}

export function useBackendConfig() {
  const context = useContext(BackendConfigContext);
  if (!context) {
    throw new Error("useBackendConfig must be used inside BackendConfigProvider");
  }
  return context;
}

function stripTrailingSlash(value: string) {
  return value.replace(/\/+$/, "");
}

function storageAvailable() {
  return typeof window !== "undefined" && "localStorage" in window;
}
