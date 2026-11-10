import { language as bash } from "@twinkleplop/bash";
import { language as toml } from "@twinkleplop/toml";

import * as utils from "./utils";

import type { Build } from "bongo";
import type { Fence } from "@bongo/markdown";

const NAMES: Record<string, string> = {
  bash: "shell",
  sh: "shell",
  toml: "toml",
};

/**
 * The twinkleplop highlighters for the fence languages that the docs use.
 * @example markdown({ highlighter: twinkleplop({ languages }) })
 */
export const languages = {
  bash: bash(),
  toml: toml(),
};

const PROMPT = /(?<=^|<code>)<span class="l"><span class="tok operator">\$<\/span> /gm;
const TABLE = /(?<=^|<code>)<span class="l">(<span class="tok punctuation">\[\[?<\/span>.*<span class="tok punctuation">\]\]?<\/span>)<\/span>$/gm;

/** What the copy button copies: for a transcript, its commands without the `$ `. */
function copied(code: string): string {
  let lines = code.trimEnd().split("\n");
  if (!lines.some((line) => line.startsWith("$ "))) return code.trimEnd();
  return lines
    .filter((line) => line.startsWith("$ "))
    .map((line) => line.slice(2).replace(/\s+#.*$/, ""))
    .join("\n");
}

function frame(html: string, lang: string, code: string, filename?: string): string {
  let body = html.replace(/\n<span class="l"><\/span>(<\/code><\/pre>)$/, "$1");
  if (lang === "toml") {
    body = body.replace(TABLE, (_, inner: string) => `<span class="l"><span class="tok table">${inner.replace(/<[^>]+>/g, "")}</span></span>`);
  }
  body = body.replace(PROMPT, '<span class="l"><span class="p">$</span> ');
  let label = utils.escape(filename ?? NAMES[lang] ?? lang);
  let bar = `<div class="bar"><span>${label}</span><button class="copy" type="button" data-copy="${utils.escape(copied(code))}" aria-label="Copy the code">copy</button></div>`;
  return `<figure class="code">${label ? bar : ""}${body}</figure>`;
}

/**
 * Frame each code block with a title bar and a copy button.
 * @example markdown({ fence })
 */
export const fence: Fence = (html, block) => frame(html, block.lang, block.code, block.filename);

/**
 * A code block outside Markdown, framed as a fence frames it.
 * @example block('[keys]\nprefix = "ctrl+a"', "toml", "config.toml")
 */
export function block(code: string, lang: keyof typeof languages, title?: string): string {
  let html = languages[lang](code).replace(/^<pre class="twinkleplop">/, `<pre class="twinkleplop language-${lang}" data-language="${lang}">`);
  return frame(html, lang, code, title);
}

const SKIP = /^<(pre|script|style|code|kbd|textarea|title)\b/i;
const TERM = /[A-Za-z0-9_~/]+(?:[-+.:][A-Za-z0-9_?~/]+)+/g;
const NAME = /Herdr (desktop app|TUI|app)\b/g;
const SHORT = 32;

/**
 * Keep version numbers, flags, paths and product names on one line.
 * @example keepTerms("<p>hc 0.1.18</p>") //=> '<p>hc <span class="nw">0.1.18</span></p>'
 */
export function keepTerms(html: string): string {
  let out = "";
  let skip = "";
  let depth = 0;
  for (let part of html.split(/(<[^>]+>)/)) {
    if (part.startsWith("<")) {
      if (!skip) {
        let match = SKIP.exec(part);
        if (match && !part.endsWith("/>")) {
          skip = match[1].toLowerCase();
          depth = 1;
        }
      } else if (new RegExp(`^<${skip}\\b`, "i").test(part)) depth++;
      else if (new RegExp(`^</${skip}>`, "i").test(part) && --depth === 0) skip = "";
      out += part;
      continue;
    }
    out += skip ? part : part.replace(TERM, (term) => `<span class="nw">${term}</span>`).replace(NAME, (term) => `<span class="nw">${term}</span>`);
  }
  return out;
}

/**
 * Wrap docs tables for phones, keep short code spans whole, and keep terms on one line.
 * @example export default { $hooks: { transform } }
 */
export const transform: Build.Transform = (html, ctx) => {
  if (ctx.kind !== "page") return html;
  let body = html
    .replace(/<article class="prose">[\s\S]*<\/article>/, (prose) =>
      prose
        .replace(/<table>[\s\S]*?<\/table>/g, (table) => {
          let cols = /<thead>[\s\S]*?<\/thead>/.exec(table)?.[0].match(/<th\b/g)?.length ?? 0;
          return `<div class="tbl cols-${cols}">${table}</div>`;
        })
        .replace(/<code>([^<]*)<\/code>/g, (span, inner: string) => (inner.replace(/&\w+;/g, "x").length > SHORT ? span : `<code class="short">${inner}</code>`)),
    );
  let start = body.indexOf("<body");
  return body.slice(0, start) + keepTerms(body.slice(start));
};
