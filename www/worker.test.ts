import { afterAll, afterEach, beforeAll, describe, expect, spyOn, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";

import worker from "./worker";

import type { Mock } from "bun:test";

type Sent = { url: string; init?: RequestInit };

const SCRIPT = readFileSync(join(import.meta.dirname, "install.sh"), "utf8");

let env = {
  CROWNE_SITE: "site_123",
  CROWNE_KEY: "crw_test_abc",
  ASSETS: { fetch: async (request: Request) => new Response(`asset ${request.method} ${new URL(request.url).pathname}`) },
};

let mock: Mock<typeof fetch> | undefined;
let waits: Promise<unknown>[] = [];

let ctx = {
  waitUntil: (promise: Promise<unknown>) => void waits.push(promise),
  passThroughOnException() {},
} as unknown as ExecutionContext;

function upstream(reply: () => Response | Promise<Response>): Sent[] {
  let sent: Sent[] = [];
  let { preconnect } = globalThis.fetch;
  mock = spyOn(globalThis, "fetch").mockImplementation(
    Object.assign(
      async (input: RequestInfo | URL, init?: RequestInit) => {
        sent.push({ url: String(input), init });
        return reply();
      },
      { preconnect },
    ),
  );
  return sent;
}

function visit(path: string, init: RequestInit = {}): Request {
  let headers = new Headers(init.headers);
  if (!headers.has("cf-connecting-ip")) headers.set("cf-connecting-ip", "203.0.113.7");
  return new Request("https://rodeo.tools" + path, { ...init, headers });
}

let call = (path: string, init?: RequestInit, vars: Partial<typeof env> = {}) => worker.fetch(visit(path, init), { ...env, ...vars }, ctx);

let agent = Object.getOwnPropertyDescriptor(navigator, "userAgent");

beforeAll(() => {
  Object.defineProperty(navigator, "userAgent", { value: "Cloudflare-Workers", configurable: true });
});

afterAll(() => {
  if (agent) Object.defineProperty(navigator, "userAgent", agent);
  else Reflect.deleteProperty(navigator, "userAgent");
});

afterEach(() => {
  mock?.mockRestore();
  mock = undefined;
  waits = [];
});

describe("install script", () => {
  test("serves install.sh as a shell script, cached for five minutes", async () => {
    upstream(() => new Response(null, { status: 202 }));
    let res = await call("/install.sh");
    expect(res.status).toBe(200);
    expect(res.headers.get("content-type")).toBe("text/x-shellscript; charset=utf-8");
    expect(res.headers.get("cache-control")).toBe("public, max-age=300");
    expect(await res.text()).toBe(SCRIPT);
  });

  test("answers HEAD with the headers and no body", async () => {
    upstream(() => new Response(null, { status: 202 }));
    let res = await call("/install.sh", { method: "HEAD" });
    expect(res.headers.get("content-type")).toBe("text/x-shellscript; charset=utf-8");
    expect(await res.text()).toBe("");
  });

  test("serves the script at / to a command line", async () => {
    upstream(() => new Response(null, { status: 202 }));
    for (let ua of ["curl/8.7.1", "Wget/1.24.5", "libfetch/3.0", "HTTPie/3.2.4", "python-requests/2.32"]) {
      let res = await call("/", { headers: { "user-agent": ua } });
      expect(await res.text()).toBe(SCRIPT);
    }
  });

  test("serves the home page at / to a browser, even one that says curl", async () => {
    let sent = upstream(() => new Response(null, { status: 202 }));
    let text = async (ua?: string) => (await call("/", ua ? { headers: { "user-agent": ua } } : {})).text();
    expect(await text("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 Safari/605.1.15")).toBe("asset GET /");
    expect(await text("Mozilla/5.0 (X11; Linux x86_64) curl-like/1.0")).toBe("asset GET /");
    expect(await text()).toBe("asset GET /");
    expect(sent).toEqual([]);
  });

  test("leaves other methods on the script paths to the assets", async () => {
    let sent = upstream(() => new Response(null, { status: 202 }));
    expect(await (await call("/install.sh", { method: "POST", body: "x" })).text()).toBe("asset POST /install.sh");
    expect(sent).toEqual([]);
  });
});

describe("download event", () => {
  test("counts a download, with whether it was piped and the platform", async () => {
    let sent = upstream(() => new Response(null, { status: 202 }));
    await call("/", { headers: { "user-agent": "curl/8.7.1 (x86_64-pc-linux-gnu)" } });
    await call("/install.sh", { headers: { "user-agent": "curl/8.4.0 (Darwin)" } });
    await Promise.all(waits);
    expect(sent.map((s) => s.url)).toEqual(["https://in.crowne.io/v1/events?site=site_123", "https://in.crowne.io/v1/events?site=site_123"]);
    let events = await Promise.all(sent.map(async (s) => JSON.parse(await new Response(s.init?.body).text())[0]));
    expect(events.map((e) => [e.n, e.u, e.p])).toEqual([
      ["download", "https://rodeo.tools/", { piped: true, platform: "linux" }],
      ["download", "https://rodeo.tools/install.sh", { piped: false, platform: "macos" }],
    ]);
    expect(new Headers(sent[0].init?.headers).get("authorization")).toBe("Bearer crw_test_abc");
  });

  test("never waits for crowne to answer", async () => {
    upstream(() => new Promise<Response>(() => {}));
    let res = await Promise.race([call("/install.sh"), Bun.sleep(200).then(() => null)]);
    expect(res).not.toBeNull();
    expect(await res!.text()).toBe(SCRIPT);
    expect(waits.length).toBe(1);
  });

  test("still serves the script when crowne fails", async () => {
    let warn = spyOn(console, "warn").mockImplementation(() => {});
    try {
      upstream(() => Promise.reject(new Error("offline")));
      let res = await call("/install.sh");
      expect(await res.text()).toBe(SCRIPT);
      mock?.mockRestore();
      upstream(() => new Response("nope", { status: 500 }));
      res = await call("/install.sh");
      expect(await res.text()).toBe(SCRIPT);
      await Promise.all(waits);
      expect(warn).toHaveBeenCalledTimes(2);
    } finally {
      warn.mockRestore();
    }
  });

  test("counts nothing without a key or a site", async () => {
    let sent = upstream(() => new Response(null, { status: 202 }));
    expect(await (await call("/install.sh", {}, { CROWNE_KEY: "" })).text()).toBe(SCRIPT);
    expect(await (await call("/install.sh", {}, { CROWNE_SITE: "" })).text()).toBe(SCRIPT);
    expect(sent).toEqual([]);
    expect(waits).toEqual([]);
  });
});

describe("analytics proxy", () => {
  test("serves the crowne script", async () => {
    let sent = upstream(() => new Response("(()=>{})()"));
    let res = await call("/x/script.js");
    expect(sent.map((s) => s.url)).toEqual(["https://in.crowne.io/v1/script.js"]);
    expect(res.headers.get("content-type")).toBe("text/javascript; charset=utf-8");
    expect(await res.text()).toBe("(()=>{})()");
  });

  test("sends events on with the key", async () => {
    let sent = upstream(() => Response.json({ accepted: 1 }, { status: 202 }));
    let body = '[{"n":"pageview","u":"https://rodeo.tools/docs/keys/"}]';
    let res = await call("/x", { method: "POST", body });
    expect(res.status).toBe(202);
    expect(sent[0].url).toBe("https://in.crowne.io/v1/events?site=site_123");
    expect(await new Response(sent[0].init?.body).text()).toBe(body);
  });

  test("serves every other path from the assets", async () => {
    let sent = upstream(() => new Response(null, { status: 500 }));
    let text = async (path: string, init?: RequestInit) => (await call(path, init)).text();
    expect(await text("/docs/keys/")).toBe("asset GET /docs/keys/");
    expect(await text("/x")).toBe("asset GET /x");
    expect(await text("/x/script.js", { method: "POST", body: "x" })).toBe("asset POST /x/script.js");
    expect(sent).toEqual([]);
  });
});
