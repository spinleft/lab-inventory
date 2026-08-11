import { http, HttpResponse } from "msw";
import { describe, expect, it } from "vitest";
import { server } from "../test/server";
import { ApiError, createApiClient } from "./httpClient";

const BASE_URL = "http://api.test/api/v1";
const client = createApiClient(BASE_URL);

describe("createApiClient", () => {
  it("strips trailing slashes from the base url", async () => {
    server.use(http.get(`${BASE_URL}/ping`, () => HttpResponse.json({ ok: true })));

    await expect(createApiClient(`${BASE_URL}///`).get("/ping")).resolves.toEqual({
      ok: true,
    });
  });

  it("joins paths whether or not they are prefixed with a slash", async () => {
    server.use(http.get(`${BASE_URL}/ping`, () => HttpResponse.json({ ok: true })));

    await expect(client.get("ping")).resolves.toEqual({ ok: true });
  });

  it("serialises query values and drops empty ones", async () => {
    let seen: string | undefined;
    server.use(
      http.get(`${BASE_URL}/items`, ({ request }) => {
        seen = new URL(request.url).search;
        return HttpResponse.json([]);
      }),
    );

    await client.get("/items", {
      blank: "",
      keyword: "光谱仪",
      limit: 20,
      missing: null,
      undef: undefined,
      withStock: true,
    });

    const params = new URLSearchParams(seen);
    expect(params.get("keyword")).toBe("光谱仪");
    expect(params.get("limit")).toBe("20");
    expect(params.get("withStock")).toBe("true");
    expect(params.has("blank")).toBe(false);
    expect(params.has("missing")).toBe(false);
    expect(params.has("undef")).toBe(false);
  });

  it("sends json bodies with a json content type", async () => {
    let contentType: string | null = null;
    let body: unknown;
    server.use(
      http.post(`${BASE_URL}/items`, async ({ request }) => {
        contentType = request.headers.get("content-type");
        body = await request.json();
        return HttpResponse.json({ created: true });
      }),
    );

    await expect(client.post("/items", { name: "示波器" })).resolves.toEqual({
      created: true,
    });
    expect(contentType).toBe("application/json");
    expect(body).toEqual({ name: "示波器" });
  });

  it("omits the content type when there is no body", async () => {
    let contentType: string | null = "unset";
    server.use(
      http.post(`${BASE_URL}/logout`, ({ request }) => {
        contentType = request.headers.get("content-type");
        return HttpResponse.json({ ok: true });
      }),
    );

    await client.post("/logout");
    expect(contentType).toBeNull();
  });

  it("lets the browser set the boundary for form data", async () => {
    let contentType: string | null = null;
    server.use(
      http.post(`${BASE_URL}/uploads`, ({ request }) => {
        contentType = request.headers.get("content-type");
        return HttpResponse.json({ upload_id: "1" });
      }),
    );

    const form = new FormData();
    form.set("file", new Blob(["data"]), "a.txt");
    await client.postFormData("/uploads", form);

    expect(contentType).toMatch(/^multipart\/form-data; boundary=/);
  });

  it("supports patch and delete", async () => {
    server.use(
      http.patch(`${BASE_URL}/items/1`, () => HttpResponse.json({ patched: true })),
      http.delete(`${BASE_URL}/items/1`, () => new HttpResponse(null, { status: 204 })),
    );

    await expect(client.patch("/items/1", { name: "x" })).resolves.toEqual({
      patched: true,
    });
    await expect(client.delete("/items/1")).resolves.toBeNull();
  });

  it("returns plain text for non-json responses", async () => {
    server.use(http.get(`${BASE_URL}/version`, () => HttpResponse.text("v1.2.3")));

    await expect(client.get("/version")).resolves.toBe("v1.2.3");
  });

  it("raises ApiError carrying the backend error message", async () => {
    server.use(
      http.get(`${BASE_URL}/items`, () =>
        HttpResponse.json({ error: "没有权限" }, { status: 403 }),
      ),
    );

    await expect(client.get("/items")).rejects.toMatchObject({
      message: "没有权限",
      name: "ApiError",
      status: 403,
    });
    await expect(client.get("/items")).rejects.toBeInstanceOf(ApiError);
  });

  it("falls back to a text body as the error message", async () => {
    server.use(
      http.get(`${BASE_URL}/items`, () =>
        HttpResponse.text("upstream exploded", { status: 502 }),
      ),
    );

    await expect(client.get("/items")).rejects.toMatchObject({
      message: "upstream exploded",
      status: 502,
    });
  });

  it("falls back to the status text when the body carries no message", async () => {
    server.use(
      http.get(`${BASE_URL}/items`, () =>
        HttpResponse.json({ detail: "nope" }, { status: 500, statusText: "Server Error" }),
      ),
    );

    await expect(client.get("/items")).rejects.toMatchObject({
      message: "Server Error",
      status: 500,
    });
  });

  describe("downloadBlob", () => {
    it("reads the file name out of the content disposition header", async () => {
      server.use(
        http.get(`${BASE_URL}/export`, () =>
          HttpResponse.text("id,name", {
            headers: { "content-disposition": 'attachment; filename="assets.csv"' },
          }),
        ),
      );

      const download = await client.downloadBlob("/export");
      expect(download.fileName).toBe("assets.csv");
      expect(await download.blob.text()).toBe("id,name");
    });

    it("returns a null file name when the header is absent or unparseable", async () => {
      server.use(
        http.get(`${BASE_URL}/export`, () => HttpResponse.text("id,name")),
        http.get(`${BASE_URL}/export-odd`, () =>
          HttpResponse.text("id,name", {
            headers: { "content-disposition": "attachment" },
          }),
        ),
      );

      expect((await client.downloadBlob("/export")).fileName).toBeNull();
      expect((await client.downloadBlob("/export-odd")).fileName).toBeNull();
    });

    it("forwards query parameters and raises ApiError on failure", async () => {
      let seen: string | undefined;
      server.use(
        http.get(`${BASE_URL}/export`, ({ request }) => {
          seen = new URL(request.url).searchParams.get("format") ?? undefined;
          return HttpResponse.json({ error: "导出失败" }, { status: 400 });
        }),
      );

      await expect(client.downloadBlob("/export", { format: "csv" })).rejects.toMatchObject({
        message: "导出失败",
        status: 400,
      });
      expect(seen).toBe("csv");
    });
  });
});
