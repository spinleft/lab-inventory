export class ApiError extends Error {
  constructor(
    public readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

type RequestOptions = {
  body?: unknown;
  method?: string;
};

export function createApiClient(apiBaseUrl: string) {
  const baseUrl = apiBaseUrl.replace(/\/+$/, "");

  async function request<T>(path: string, options: RequestOptions = {}) {
    const response = await fetch(`${baseUrl}/${path.replace(/^\/+/, "")}`, {
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
      credentials: "include",
      headers:
        options.body === undefined
          ? undefined
          : {
              "Content-Type": "application/json",
            },
      method: options.method ?? "GET",
    });

    const payload = await readPayload(response);
    if (!response.ok) {
      throw new ApiError(response.status, readErrorMessage(payload, response.statusText));
    }

    return payload as T;
  }

  return {
    get: <T = unknown>(path: string) => request<T>(path),
    patch: <T = unknown>(path: string, body: unknown) =>
      request<T>(path, { body, method: "PATCH" }),
    post: <T = unknown>(path: string, body?: unknown) =>
      request<T>(path, { body, method: "POST" }),
  };
}

async function readPayload(response: Response) {
  if (response.status === 204) {
    return null;
  }

  const contentType = response.headers.get("content-type") ?? "";
  if (contentType.includes("application/json")) {
    return response.json();
  }

  return response.text();
}

function readErrorMessage(payload: unknown, fallback: string) {
  if (typeof payload === "object" && payload !== null && "error" in payload) {
    const error = (payload as { error?: unknown }).error;
    if (typeof error === "string") {
      return error;
    }
  }

  if (typeof payload === "string" && payload.trim()) {
    return payload;
  }

  return fallback || "请求失败。";
}
