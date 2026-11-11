# Releasing

How to cut a release, what each workflow makes, and the one-time setup they need.

## Cut a release

1. Set `version` in `[workspace.package]` in the root `Cargo.toml`. Run `cargo check` so `Cargo.lock` follows.
2. Run `mise run check`.
3. Commit, for example `chore: release v0.2.0`.
4. Tag that commit and push: `git tag -a v0.2.0 -m v0.2.0 && git push origin main v0.2.0`.

A tag always sits on its own release commit, the one that sets the version. Never tag another commit, even when the version is already right: bump it.

The tag starts `.github/workflows/release.yml`. The tag must be `v` plus the `rodeo` package version, or the `cli` jobs stop.

To run a release again for a tag that exists: `gh workflow run release.yml --ref v0.2.0 -f tag=v0.2.0`. Run it from the tag: the `homebrew` job uses the `release` environment, which takes only `v*` tags. It builds the tag's commit, keeps the GitHub release if there is one, and replaces its assets and their R2 copies. It moves `latest` in R2 only when the tag is the newest.

## What `release.yml` makes

| Job | Output |
| --- | --- |
| `cli` | `rodeo-<target>.tar.gz` with `rodeo` alone at the archive root, and `rodeo-<target>.tar.gz.sha256` (`<hex>  <file>`), for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`. |
| `release` | The GitHub release with generated notes and every archive and `.sha256`. Then `jdx/packslip` attests the archives' build provenance, signs a release manifest with the job's OIDC identity, and uploads `packslip.sigstore.json`. The manifest names `rodeo` as the binary and `hc` 0.1.18 or newer as a required command. |
| `r2` | Each archive and `.sha256` in the `rodeo-releases` R2 bucket at `<tag>/<file>`, then `latest`, a text object with the tag. It uploads through the R2 S3 API with `curl --aws-sigv4`, because that API honors the token's bucket scope and the REST API that `wrangler r2` calls does not. It moves `latest` only to a newer tag, by `sort -V`. |
| `homebrew` | `Formula/rodeo.rb` in `lukeed/homebrew-tap`, written from `.github/rodeo.rb.tmpl` with the version and the four sha256s, committed as `chore: rodeo <version>`. It runs after `r2`, because the formula downloads from `rodeo.lukeed.com`. |
| `site` | Calls `www.yml` with the tag, after `homebrew`, so the site shows the new version only once every installer can fetch it. If an earlier job fails, the site stays on the last release. |

The Linux builds use `cargo zigbuild` and are static musl binaries. `libghostty-vt-sys` builds Ghostty with zig and maps the Rust target to a zig target itself, so they need no extra env. It builds for the `baseline` CPU unless `LIBGHOSTTY_VT_SYS_CPU` is set, so the binaries run on any CPU of their arch. The macOS x86_64 build cross-compiles on the arm64 runner. The macOS jobs run on `macos-15`: `libghostty-vt-sys` needs zig 0.15.2, which cannot link against the Xcode 26.4 SDK of `macos-26`. Move them when a zig release that `libghostty-vt-sys` accepts fixes that. Only `aarch64-apple-darwin` and `x86_64-unknown-linux-musl` run `rodeo --version` in CI.

These install paths read the release:

- `curl -fsSL https://rodeo.lukeed.com | sh`: `www/install.sh` reads the tag from `https://rodeo.lukeed.com/download/latest`, or takes `RODEO_VERSION`, then gets the archive and its `.sha256` from `https://rodeo.lukeed.com/download/<tag>/`. The Worker serves both from R2. It does not use GitHub.
- `mise use -g packslip:github.com/lukeed/rodeo`: reads the packslip from the GitHub release.
- `brew install lukeed/tap/rodeo`: reads the formula, which downloads from `https://rodeo.lukeed.com/download/<tag>/`.

## Other workflows

- `ci.yml`: on pushes to `main` and on pull requests, except for changes only under `www/`. `lint` runs `jactionlint`, `cargo fmt --check` and clippy with `-D warnings`. `test` runs `cargo nextest run --workspace --locked` on `ubuntu-latest` and `macos-15`, with `tmux` and the pinned, checksum-checked `hc`. To use a newer `hc`, change `HC_VERSION` and both `hc_sha256` values.
- `www.yml`: deploys the site. It runs `bun test`, `bun run build` and `bun run lint` in `www/`, then `wrangler deploy`. It runs in three ways:
  - From `release.yml`, as its last job, built from the tag.
  - On pushes to `main` that change `www/`, `mise.toml` (the bun pin) or the workflow itself. A version bump alone does not start it: the release does. Before it builds, it takes `Cargo.toml` (the version), `crates/rodeo/src/cli.rs` (the usage) and `crates/config/src/default.toml` (the config page) from the latest `v*` tag, so a site change between releases never shows an unreleased version, usage or config.
  - By hand (`gh workflow run www.yml`), the same as a push.

## One-time setup

### GitHub repo settings

- Settings > Secrets and variables > Actions: add the secrets below.
- Settings > Actions > General: if you limit which actions can run, allow `actions/*`, `jdx/*`, `Swatinem/rust-cache` and `cloudflare/wrangler-action`. packslip calls `actions/attest-build-provenance`.
- Settings > Actions > General > Workflow permissions: "Read repository contents" is enough. Each release job asks for the write permissions it needs.
- Settings > General > Releases: keep "immutable releases" off. The `release` job publishes the release, then uploads the archives and the packslip.
- Settings > Rules: add a tag ruleset for `v*` so only you can create or move release tags.
- Settings > Environments: add `release`, with "Selected branches and tags" and one tag rule, `v*`. It holds `HOMEBREW_TAP_SSH_KEY`.
- If you make `lint` or `test` required checks, a pull request that changes only `www/` never reports them. Leave them optional, or drop `paths-ignore` from `ci.yml`.

### Secrets

| Secret | Used by | What it is |
| --- | --- | --- |
| `HOMEBREW_TAP_SSH_KEY` | `release.yml` `homebrew` | A secret of the `release` environment, not of the repo. It is the private half of an ed25519 deploy key with write access on `lukeed/homebrew-tap` only. The tap checkout uses it, so `git push` goes over SSH. `jactionlint` rejects a key that does not expire (`checkout-static-credentials`); the workflow ignores that rule on this line only, because the key reaches one repo and only `v*` tags can read it. To replace it: `ssh-keygen -t ed25519 -N "" -f key`, `gh repo deploy-key add key.pub -R lukeed/homebrew-tap --allow-write`, `gh secret set HOMEBREW_TAP_SSH_KEY -R lukeed/rodeo --env release < key`, delete the old deploy key, then delete `key`. |
| `CLOUDFLARE_API_TOKEN` | `www.yml`, `release.yml` `r2` and `site` | A Cloudflare user API token with: Workers Scripts Write and Account Settings Read on the account; Workers Routes Write and Zone Read on the `lukeed.com` zone; Workers R2 Storage Bucket Item Read and Write on the `rodeo-releases` bucket only; Memberships Read and User Details Read. |
| `CLOUDFLARE_ACCOUNT_ID` | `www.yml`, `release.yml` `r2` and `site` | The Cloudflare account ID. |

One Actions variable goes with the token:

| Variable | Used by | What it is |
| --- | --- | --- |
| `CLOUDFLARE_API_TOKEN_ID` | `release.yml` `r2` | The ID of `CLOUDFLARE_API_TOKEN`. It is the R2 S3 access key ID; the secret key is the sha256 of the token. When you replace the token, change both. |

### packslip

It needs no secret. It signs with the `release` job's OIDC token (`id-token: write`) and attests with `attestations: write`. Attestations on a private repo need GitHub Enterprise Cloud, so the repo must be public before the first release.

### Cloudflare

The Worker serves releases from the `rodeo-releases` R2 bucket, bound as `RELEASES` in `www/wrangler.jsonc`. Make it once with `bunx wrangler r2 bucket create rodeo-releases`, and keep its `r2.dev` URL off.

The Worker reads `CROWNE_KEY` as a Worker secret. Set it once with `bunx wrangler secret put CROWNE_KEY` in `www/`. The site id is `CROWNE_SITE` in `www/wrangler.jsonc`. The `lukeed.com` zone must be in the same Cloudflare account for the custom domain route.
