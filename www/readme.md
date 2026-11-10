# www

The site at [rodeo.lukeed.com](https://rodeo.lukeed.com), built with [bongo](https://bongo.build) and served from Cloudflare Workers static assets.

```sh
bun install
bun run dev        # http://localhost:3000
bun run build      # writes public/
bun run preview    # wrangler dev, with the Worker
bun run deploy     # build, then wrangler deploy
bun run test       # the Worker and install.sh
bun run lint       # seolint over public/
```

## Layout

| Path | What it is |
| --- | --- |
| `content/` | Pages. `index.hbs` is the home page; `docs/*.md` are the docs. |
| `layouts/` | `html.hbs` wraps every page; `docs/index.hbs` is the docs shell. |
| `partials/` | The nav, the footer, the installer tabs, `rodeo --help`, the mode bars, and `source.ts`, which prints a repo file as a code block. |
| `assets/` | Styles, the client script, the font and images. `js/world.ts` draws the pixel scenes, `js/tui.ts` the rodeo screen. |
| `data/` | `home.ts` holds the home page copy. `release.ts` reads the version from `Cargo.toml` and the help text from `crates/rodeo/src/cli.rs`. |
| `worker.ts` | The analytics proxy and the install script. |

The docs sidebar comes from each page's `group` and `order` frontmatter. The group names and their order live in `GROUPS` in `utils.ts`.

The Config page prints `crates/config/src/default.toml` at build time, and the `rodeo --help` panels read `cli.rs`, so both follow the code.

## The Worker

Everything is a static asset except four paths, listed in `run_worker_first`:

- `/x/script.js` and `/x` proxy [crowne.io](https://crowne.io), so a blocker is less likely to drop them.
- `/install.sh` serves the install script.
- `/` serves the install script to a command line (`curl`, `wget`, …) and the home page to a browser.

Serving the script counts a `download` event, with `piped` and `platform`. The event never blocks the response: a failed event is logged and dropped.

## Before deploying

1. Set the crowne.io key as a secret: `bunx wrangler secret put CROWNE_KEY`. The site id is `CROWNE_SITE` in `wrangler.jsonc`.
2. Point `rodeo.lukeed.com` at the Worker, which the `routes` entry claims as a custom domain.

Without the key the Worker still serves everything; it counts nothing.

## Fonts

Geist Mono, under the SIL Open Font License 1.1: `assets/fonts/geist-mono.OFL.txt`.
