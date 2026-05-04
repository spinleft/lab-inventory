export type ApiClient = ReturnType<typeof createApiClient>;

type RequestOptions = {
  method?: "GET" | "POST" | "PATCH" | "PUT" | "DELETE";
  body?: unknown;
  signal?: AbortSignal;
};

export class ApiError extends Error {
  status: number;
  payload: unknown;

  constructor(status: number, message: string, payload: unknown) {
    super(message);
    this.name = "ApiError";
    this.status = status;
    this.payload = payload;
  }
}

export function createApiClient(apiBaseUrl: string) {
  return {
    get: <T>(path: string, options?: Pick<RequestOptions, "signal">) =>
      request<T>(apiBaseUrl, path, { ...options, method: "GET" }),
    post: <T>(path: string, body?: unknown, options?: Pick<RequestOptions, "signal">) =>
      request<T>(apiBaseUrl, path, { ...options, method: "POST", body }),
    patch: <T>(path: string, body?: unknown, options?: Pick<RequestOptions, "signal">) =>
      request<T>(apiBaseUrl, path, { ...options, method: "PATCH", body }),
    delete: <T>(path: string, options?: Pick<RequestOptions, "signal">) =>
      request<T>(apiBaseUrl, path, { ...options, method: "DELETE" }),
  };
}

async function request<T>(
  apiBaseUrl: string,
  path: string,
  { method = "GET", body, signal }: RequestOptions,
) {
  const response = await fetch(`${apiBaseUrl}${withLeadingSlash(path)}`, {
    method,
    credentials: "include",
    signal,
    headers:
      body === undefined
        ? undefined
        : {
            "Content-Type": "application/json",
          },
    body: body === undefined ? undefined : JSON.stringify(body),
  });

  const payload = await readPayload(response);
  if (!response.ok) {
    throw new ApiError(response.status, getErrorMessage(payload), payload);
  }

  return payload as T;
}

async function readPayload(response: Response) {
  if (response.status === 204) {
    return null;
  }

  const text = await response.text();
  if (!text) {
    return null;
  }

  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function getErrorMessage(payload: unknown) {
  if (
    payload &&
    typeof payload === "object" &&
    "error" in payload &&
    typeof payload.error === "string"
  ) {
    return payload.error;
  }
  return "请求失败。";
}

function withLeadingSlash(path: string) {
  return path.startsWith("/") ? path : `/${path}`;
}
