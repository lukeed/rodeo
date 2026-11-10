import { crowne } from "crowne/server";

// @ts-expect-error — wrangler's Text module rule turns this into a string.
import script from "./install.sh";

import type { Props } from "crowne/server";

type Stored = { size: number; httpEtag: string; body?: ReadableStream };

type Env = {
  CROWNE_SITE: string;
  CROWNE_KEY: string;
  ASSETS: {
    fetch(request: Request): Promise<Response>;
  };
  /** The `rodeo-releases` bucket: `latest` names the newest tag; `<tag>/<archive>` holds each file. */
  RELEASES: {
    head(key: string): Promise<Stored | null>;
    get(key: string): Promise<Required<Stored> | null>;
  };
};

/** Anchored, so a browser whose user agent merely contains "curl" still gets the page. */
const CLI = /^(curl|wget|libfetch|httpie|python-requests|powershell|fetch)/i;

const RELEASE = /^\/download\/(v\d+\.\d+\.\d+[\w.+-]*)\/(rodeo-([a-z\d_-]+)\.tar\.gz(\.sha256)?)$/;

const SCRIPT_HEADERS = {
  "content-type": "text/x-shellscript; charset=utf-8",
  "cache-control": "public, max-age=300",
};

const TEXT = "text/plain; charset=utf-8";

export default {
  async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    let { pathname } = new URL(req.url);

    let isEvent = req.method === "POST" && pathname === "/x";
    let isScript = req.method === "GET" && pathname === "/x/script.js";
    if (isScript || isEvent) {
      return crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).proxy(req);
    }

    let isRead = req.method === "GET" || req.method === "HEAD";

    if (isRead && pathname === "/download/latest") {
      return release(req, env, "latest", { "content-type": TEXT, "cache-control": "public, max-age=60" });
    }

    let match = isRead && RELEASE.exec(pathname);
    if (match) {
      let [, version, file, target, sum] = match;
      let res = await release(req, env, `${version}/${file}`, {
        "content-type": sum ? TEXT : "application/gzip",
        "cache-control": "public, max-age=31536000, immutable",
      });
      if (res.ok && !sum && req.method === "GET") count(req, env, ctx, "download", { version, target });
      return res;
    }

    let agent = req.headers.get("user-agent") ?? "";
    let wantsScript = pathname === "/install.sh" || (pathname === "/" && CLI.test(agent));

    if (wantsScript && isRead) {
      // Reading the installer is an attempt, not a download: the archive it then fetches is the
      // install, and counting both as a download counts one install twice. A HEAD is neither.
      if (req.method === "GET") count(req, env, ctx, "installer", { piped: pathname === "/", platform: platform(agent) });
      return new Response(req.method === "HEAD" ? null : script, {
        headers: SCRIPT_HEADERS,
      });
    }

    return env.ASSETS.fetch(req);
  },
};

/** Streams one object from the releases bucket, or a 404 that no cache keeps. */
async function release(req: Request, env: Env, key: string, headers: Record<string, string>): Promise<Response> {
  let object = req.method === "HEAD" ? await env.RELEASES.head(key) : await env.RELEASES.get(key);
  if (!object) {
    return new Response("not found\n", { status: 404, headers: { "content-type": TEXT, "cache-control": "no-store" } });
  }
  return new Response(object.body ?? null, {
    headers: { ...headers, etag: object.httpEtag, "content-length": String(object.size) },
  });
}

/** Sends a crowne event in the background, so a slow or failed crowne never delays the response. */
function count(req: Request, env: Env, ctx: ExecutionContext, name: string, props: Props) {
  if (!env.CROWNE_KEY || !env.CROWNE_SITE) return;
  let event = crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).track(req, name, props);
  ctx.waitUntil(event.catch((error: unknown) => console.warn(`crowne: ${name} event`, error)));
}

function platform(agent: string): string {
  if (/darwin|mac/i.test(agent)) return "macos";
  if (/linux/i.test(agent)) return "linux";
  if (/win/i.test(agent)) return "windows";
  return "unknown";
}
