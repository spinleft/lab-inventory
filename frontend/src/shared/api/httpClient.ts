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
  query?: Record<string, string | number | boolean | null | undefined>;
};

export type BlobDownload = {
  blob: Blob;
  fileName: string | null;
};

export function createApiClient(apiBaseUrl: string) {
  const baseUrl = apiBaseUrl.replace(/\/+$/, "");

  async function request<T>(path: string, options: RequestOptions = {}) {
    const url = buildUrl(baseUrl, path, options.query);
    const isFormData = options.body instanceof FormData;
    let body: BodyInit | null | undefined;
    if (options.body === undefined) {
      body = undefined;
    } else if (isFormData) {
      body = options.body as FormData;
    } else {
      body = JSON.stringify(options.body);
    }
    const response = await fetch(url, {
      body,
      credentials: "include",
      headers:
        options.body === undefined || isFormData
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

  async function downloadBlob(
    path: string,
    query?: RequestOptions["query"],
  ): Promise<BlobDownload> {
    const response = await fetch(buildUrl(baseUrl, path, query), {
      credentials: "include",
      method: "GET",
    });
    if (!response.ok) {
      const payload = await readPayload(response);
      throw new ApiError(response.status, readErrorMessage(payload, response.statusText));
    }

    return {
      blob: await response.blob(),
      fileName: readFileName(response.headers.get("content-disposition")),
    };
  }

  return {
    delete: <T = unknown>(path: string) => request<T>(path, { method: "DELETE" }),
    downloadBlob,
    get: <T = unknown>(path: string, query?: RequestOptions["query"]) =>
      request<T>(path, { query }),
    patch: <T = unknown>(path: string, body: unknown) =>
      request<T>(path, { body, method: "PATCH" }),
    post: <T = unknown>(path: string, body?: unknown) =>
      request<T>(path, { body, method: "POST" }),
    postFormData: <T = unknown>(path: string, body: FormData) =>
      request<T>(path, { body, method: "POST" }),
  };
}

function buildUrl(
  baseUrl: string,
  path: string,
  query?: Record<string, string | number | boolean | null | undefined>,
) {
  const url = new URL(`${baseUrl}/${path.replace(/^\/+/, "")}`);
  for (const [key, value] of Object.entries(query ?? {})) {
    if (value !== null && value !== undefined && value !== "") {
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
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

function readFileName(contentDisposition: string | null) {
  if (!contentDisposition) {
    return null;
  }
  const match = /filename="([^"]+)"/i.exec(contentDisposition);
  return match?.[1] ?? null;
}
