---
title: Install
summary: Put the `rodeo` binary on your PATH with one of three installers, or build it from source. rodeo runs on Linux and macOS, on top of `hc`.
group: Get started
order: 1
---

> rodeo has no tagged release yet. Until the first one, [build it from source](#from-source). The installers below start to work when the first release ships.

## Before you start

rodeo is a client of herdr-core. It needs `hc` 0.1.18 or newer on your PATH, on Linux or macOS. If you use the Herdr desktop app, you have `hc` already. Else, install it:

```sh
$ curl -fsSL https://releases.herdr.dev/core/install.sh | sh
$ hc --version
```

You do not need to start `hc` yourself. rodeo starts the core when it is not running.

## The install script

```sh
$ curl -fsSL https://rodeo.tools/install.sh | sh
```

The script finds your system and CPU, downloads the matching archive from the GitHub releases of `lukeed/rodeo`, checks its sha256, and puts `rodeo` in `~/.local/bin`. It tells you when that folder is not on your PATH, and when `hc` is missing or too old.

Two variables change what it does:

| Variable | Default | Does |
| --- | --- | --- |
| `RODEO_VERSION` | the latest release | Install this release, such as `v0.1.0`. |
| `RODEO_INSTALL` | `~/.local/bin` | Put the binary in this folder. |

```sh
$ curl -fsSL https://rodeo.tools/install.sh | RODEO_VERSION=v0.1.0 sh
```

Read the script before you run it: `curl -fsSL https://rodeo.tools/install.sh` prints it.

The checksum comes from the same place as the archive. It finds a broken download, not a hostile one. For a signed release manifest, use packslip or mise.

## Homebrew

```sh
$ brew install lukeed/tap/rodeo
```

## mise and packslip

Every release carries `packslip.sigstore.json`, a [packslip](https://packslip.dev) manifest. The release workflow signs it with its own identity, with no long-lived key. It states the digest, the platform and the executables of each archive. mise reads it through its packslip backend:

```sh
$ mise use -g packslip:github.com/lukeed/rodeo
```

Or use packslip on its own:

```sh
$ packslip install github.com/lukeed/rodeo
```

## Release archives

Each release has one archive per platform, with a `.sha256` file beside it. The archive holds the `rodeo` binary only.

| Platform | Archive |
| --- | --- |
| macOS, Apple silicon | `rodeo-aarch64-apple-darwin.tar.gz` |
| macOS, Intel | `rodeo-x86_64-apple-darwin.tar.gz` |
| Linux, x86-64 | `rodeo-x86_64-unknown-linux-musl.tar.gz` |
| Linux, ARM64 | `rodeo-aarch64-unknown-linux-musl.tar.gz` |

The Linux builds link against musl, so they run on any distribution.

## From source

You need [mise](https://mise.jdx.dev) for the pinned Rust and zig versions:

```sh
$ git clone https://git.nexthub.sh/lukeed/rodeo.git
$ cd rodeo
$ mise install
$ cargo build --release
$ cargo install --path crates/rodeo --locked   # optional: put it on your PATH
```

The binary is `target/release/rodeo`, or `~/.cargo/bin/rodeo` after the last step.

## Check it

```sh
$ rodeo --version
rodeo 0.1.0
```

`rodeo --help` lists every command:

<bongo:helpbox />

Next, [open rodeo](/docs/usage/).
