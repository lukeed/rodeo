# Releasing

How to cut a release, what each workflow makes, and the one-time setup they need.

## Cut a release

1. Set `version` in `[workspace.package]` in the root `Cargo.toml`. Run `cargo check` so `Cargo.lock` follows.
2. Run `mise run check`.
3. Commit, for example `chore: release v0.2.0`.
4. Tag and push: `git tag v0.2.0 && git push origin main v0.2.0`.

The tag starts `.github/workflows/release.yml`. The tag must be `v` plus the `rodeo` package version, or the `cli` jobs stop.

To run a release again for a tag that exists: `gh workflow run release.yml -f tag=v0.2.0`. It builds the tag's commit, keeps the GitHub release if there is one, and replaces its assets.

## What `release.yml` makes

| Job | Output |
| --- | --- |
| `cli` | `rodeo-<target>.tar.gz` with `rodeo` alone at the archive root, and `rodeo-<target>.tar.gz.sha256` (`<hex>  <file>`), for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`. |
| `release` | The GitHub release with generated notes and every archive and `.sha256`. Then `jdx/packslip` attests the archives' build provenance, signs a release manifest with the job's OIDC identity, and uploads `packslip.sigstore.json`. The manifest names `rodeo` as the binary and `hc` 0.1.18 or newer as a required command. |
| `homebrew` | `Formula/rodeo.rb` in `lukeed/homebrew-tap`, written from `.github/rodeo.rb.tmpl` with the version and the four sha256s, committed as `chore: rodeo <version>`. |

The Linux builds use `cargo zigbuild` and are static musl binaries. `libghostty-vt-sys` builds Ghostty with zig and maps the Rust target to a zig target itself, so they need no extra env. It builds for the `baseline` CPU unless `LIBGHOSTTY_VT_SYS_CPU` is set, so the binaries run on any CPU of their arch. The macOS x86_64 build cross-compiles on the arm64 runner. Only `aarch64-apple-darwin` and `x86_64-unknown-linux-musl` run `rodeo --version` in CI.

These install paths read the release:

- `curl -fsSL https://rodeo.lukeed.com | sh`: `www/install.sh` gets the archive and its `.sha256` from `/releases/download/<tag>/` or `/releases/latest/download/`.
- `mise use -g packslip:github.com/lukeed/rodeo`: reads the packslip.
- `brew install lukeed/tap/rodeo`: reads the formula.

## Other workflows

- `ci.yml`: on pushes to `main` and on pull requests, except for changes only under `www/`. `lint` runs `jactionlint`, `cargo fmt --check` and clippy with `-D warnings`. `test` runs `cargo nextest run --workspace --locked` on `ubuntu-latest` and `macos-latest`, with `tmux` and the pinned, checksum-checked `hc`. To use a newer `hc`, change `HC_VERSION` and both `hc_sha256` values.
- `www.yml`: on pushes to `main` that change `www/`, the root `Cargo.toml` (the site shows the version), `crates/rodeo/src/cli.rs` (the site shows the usage), `mise.toml` (the bun pin) or the workflow itself. It runs `bun test`, `bun run build` and `bun run lint` in `www/`, then `wrangler deploy`.

## One-time setup

### GitHub repo settings

- Settings > Secrets and variables > Actions: add the secrets below.
- Settings > Actions > General: if you limit which actions can run, allow `actions/*`, `jdx/*`, `Swatinem/rust-cache` and `cloudflare/wrangler-action`. packslip calls `actions/attest-build-provenance`.
- Settings > Actions > General > Workflow permissions: "Read repository contents" is enough. Each release job asks for the write permissions it needs.
- Settings > General > Releases: keep "immutable releases" off. The `release` job publishes the release, then uploads the archives and the packslip.
- Settings > Rules: add a tag ruleset for `v*` so only you can create or move release tags.
- If you make `lint` or `test` required checks, a pull request that changes only `www/` never reports them. Leave them optional, or drop `paths-ignore` from `ci.yml`.

### Secrets

| Secret | Used by | What it is |
| --- | --- | --- |
| `HOMEBREW_TAP_TOKEN` | `release.yml` `homebrew` | A fine-grained token with "Contents: read and write" on `lukeed/homebrew-tap` only. |
| `CLOUDFLARE_API_TOKEN` | `www.yml` | A Cloudflare API token from the "Edit Cloudflare Workers" template, scoped to the account and the `rodeo.lukeed.com` zone. |
| `CLOUDFLARE_ACCOUNT_ID` | `www.yml` | The Cloudflare account ID. |

### packslip

It needs no secret. It signs with the `release` job's OIDC token (`id-token: write`) and attests with `attestations: write`. Attestations on a private repo need GitHub Enterprise Cloud, so the repo must be public before the first release.

### Cloudflare

The Worker reads `CROWNE_KEY` as a Worker secret. Set it once with `bunx wrangler secret put CROWNE_KEY` in `www/`. The site id is `CROWNE_SITE` in `www/wrangler.jsonc`. The `lukeed.com` zone must be in the same Cloudflare account for the custom domain route.
