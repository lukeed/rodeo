import { existsSync } from "node:fs";
import { join } from "node:path";

import type { Template } from "bongo";

export type Link = {
  title: string;
  path: string;
  group: string;
  current?: "page";
};

const GROUPS = ["Get started", "Using rodeo", "Moving over"];

const REPO = "https://github.com/lukeed/rodeo";

/**
 * The docs pages in sidebar order: by `group` in `GROUPS` order, then by `order`.
 * @example $utils.nav($pages, "/docs/keys/") //=> [{ title: 'Install', path: '/docs/install/', group: 'Get started' }, …]
 */
export function nav(pages: Template.PageInfo[], current: string): Link[] {
  return GROUPS.flatMap((group) =>
    pages
      .filter((page) => page.params.group === group)
      .toSorted((a, b) => Number(a.params.order) - Number(b.params.order))
      .map((page) => ({
        title: page.title,
        path: page.path,
        group,
        current: page.path === current ? ("page" as const) : undefined,
      })),
  );
}

/**
 * The docs pages before and after the current one, in sidebar order.
 * @example $utils.pager($pages, "/docs/usage/") //=> { prev: { title: 'Install', … }, next: { title: 'Config', … } }
 */
export function pager(pages: Template.PageInfo[], current: string): { prev?: Link; next?: Link } {
  let links = nav(pages, current);
  let index = links.findIndex((link) => link.path === current);
  if (index === -1) return {};
  return { prev: links[index - 1], next: links[index + 1] };
}

/**
 * Text that is safe in HTML, also inside an attribute.
 * @example $utils.escape('a "b" <c>') //=> 'a &quot;b&quot; &lt;c&gt;'
 */
export function escape(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

/**
 * Text with `code` spans as HTML.
 * @example $utils.inline("reads `config.toml`") //=> 'reads <code>config.toml</code>'
 */
export function inline(text: string): string {
  return escape(text).replace(/`([^`]+)`/g, "<code>$1</code>");
}

/**
 * Text without the backticks of its `code` spans, for meta tags.
 * @example $utils.plain("reads `config.toml`") //=> 'reads config.toml'
 */
export function plain(text: string): string {
  return text.replace(/`([^`]+)`/g, "$1");
}

/**
 * A shell command as HTML, with the pipe kept beside its neighbours.
 * @example $utils.command("curl x | sh") //=> 'curl x&nbsp;|&nbsp;sh'
 */
export function command(text: string): string {
  return escape(text).replace(" | ", "&nbsp;|&nbsp;");
}

/**
 * The source file of a docs page, relative to `www/`.
 * @example $utils.file("/docs/keys/") //=> 'content/docs/keys.md'
 */
export function file(path: string): string {
  let name = "content/" + path.replace(/^\/|\/$/g, "");
  return existsSync(join(import.meta.dirname, name + ".hbs")) ? name + ".hbs" : name + ".md";
}

/**
 * The GitHub URL that edits the source of a docs page.
 * @example $utils.source("/docs/keys/") //=> 'https://github.com/lukeed/rodeo/edit/main/www/content/docs/keys.md'
 */
export function source(path: string): string {
  return `${REPO}/edit/main/www/${file(path)}`;
}

/**
 * Whether this is a production build. `bongo dev` runs no Worker, so it has no crowne.io paths.
 * @example {{#if $utils.production()}}<script defer src="/x/script.js" data-endpoint="/x"></script>{{/if}}
 */
export function production(): boolean {
  return process.env.NODE_ENV === "production";
}
