import { crowne } from "crowne/server";

// @ts-expect-error — wrangler's Text module rule turns this into a string.
import script from "./install.sh";

type Env = {
  CROWNE_SITE: string;
  CROWNE_KEY: string;
  ASSETS: {
    fetch(request: Request): Promise<Response>;
  };
};

/** Anchored, so a browser whose user agent merely contains "curl" still gets the page. */
const CLI = /^(curl|wget|libfetch|httpie|python-requests|powershell|fetch)/i;

const SCRIPT_HEADERS = {
  "content-type": "text/x-shellscript; charset=utf-8",
  "cache-control": "public, max-age=300",
};

export default {
  async fetch(req: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
    let { pathname } = new URL(req.url);

    let isEvent = req.method === "POST" && pathname === "/x";
    let isScript = req.method === "GET" && pathname === "/x/script.js";
    if (isScript || isEvent) {
      return crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).proxy(req);
    }

    let agent = req.headers.get("user-agent") ?? "";
    let wantsScript = pathname === "/install.sh" || (pathname === "/" && CLI.test(agent));

    if (wantsScript && (req.method === "GET" || req.method === "HEAD")) {
      if (env.CROWNE_KEY && env.CROWNE_SITE) {
        let event = crowne({ key: env.CROWNE_KEY, site: env.CROWNE_SITE }).track(req, "download", {
          piped: pathname === "/",
          platform: platform(agent),
        });
        ctx.waitUntil(event.catch((error: unknown) => console.warn("crowne: download event", error)));
      }
      return new Response(req.method === "HEAD" ? null : script, {
        headers: SCRIPT_HEADERS,
      });
    }

    return env.ASSETS.fetch(req);
  },
};

function platform(agent: string): string {
  if (/darwin|mac/i.test(agent)) return "macos";
  if (/linux/i.test(agent)) return "linux";
  if (/win/i.test(agent)) return "windows";
  return "unknown";
}
