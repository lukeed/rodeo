#!/bin/sh
# Installs the rodeo binary. Read it before you run it:
#   curl -fsSL https://rodeo.lukeed.com
#
#   RODEO_VERSION=v0.1.0   pin a release; defaults to the latest
#   RODEO_INSTALL=~/bin    where the binary goes; defaults to ~/.local/bin
#
# It downloads from https://rodeo.lukeed.com/download/, a copy of each GitHub
# release, and checks the published sha256, which catches a truncated
# download. For a signed release manifest, use packslip or mise instead:
#   packslip install github.com/lukeed/rodeo
#   mise use -g packslip:github.com/lukeed/rodeo
#
# rodeo needs hc (herdr-core) 0.1.18 or newer on your PATH. Get it with:
#   curl -fsSL https://releases.herdr.dev/core/install.sh | sh

set -eu

BASE="https://rodeo.lukeed.com/download"
HC_MIN="0.1.18"
SOURCE="https://rodeo.lukeed.com/docs/install/#from-source"

main() {
	dest=${RODEO_INSTALL:-$HOME/.local/bin}
	target=$(detect_target)

	archive="rodeo-$target.tar.gz"
	tmp=$(mktemp -d)
	trap 'rm -rf "$tmp"' EXIT INT TERM

	if [ -n "${RODEO_VERSION-}" ]; then
		version=$RODEO_VERSION
	else
		fetch "$BASE/latest" "$tmp/latest" || die "could not find the latest release at $BASE/latest"
		version=$(cat "$tmp/latest")
	fi
	is_tag "$version" || die "\"$version\" is not a release tag, such as v0.1.0"

	say "downloading rodeo $version for $target"
	fetch "$BASE/$version/$archive" "$tmp/$archive" || die "could not download $archive for $version.

If rodeo has not been released for your platform yet, build it from source:
  $SOURCE"
	fetch "$BASE/$version/$archive.sha256" "$tmp/$archive.sha256" || die "could not download $archive.sha256 for $version"
	verify "$tmp" "$archive"

	tar -xzf "$tmp/$archive" -C "$tmp"
	[ -f "$tmp/rodeo" ] || die "the archive did not contain a rodeo binary"

	mkdir -p "$dest"
	install -m 755 "$tmp/rodeo" "$dest/rodeo" 2>/dev/null ||
		{ cp "$tmp/rodeo" "$dest/rodeo" && chmod 755 "$dest/rodeo"; }

	say "installed $("$dest/rodeo" --version) to $dest/rodeo"

	case ":${PATH}:" in
	*":$dest:"*) ;;
	*)
		say ""
		say "$dest is not on your PATH. Add this to your shell profile:"
		say "  export PATH=\"$dest:\$PATH\""
		;;
	esac

	check_hc

	say ""
	say "next:"
	say "  rodeo                  open rodeo on the default hc session"
	say "  rodeo --session work   open it on another hc session"
	say "  https://rodeo.lukeed.com/docs/"
}

detect_target() {
	os=$(uname -s)
	arch=$(uname -m)

	case $arch in
	arm64 | aarch64) arch=aarch64 ;;
	x86_64 | amd64) arch=x86_64 ;;
	*) die "unsupported architecture: $arch; build from source: $SOURCE" ;;
	esac

	case $os in
	Darwin) echo "$arch-apple-darwin" ;;
	Linux) echo "$arch-unknown-linux-musl" ;;
	*) die "unsupported system: $os; build from source: $SOURCE" ;;
	esac
}

check_hc() {
	if ! command -v hc >/dev/null 2>&1; then
		say ""
		say "rodeo needs hc (herdr-core) $HC_MIN or newer on your PATH. Install it with:"
		say "  curl -fsSL https://releases.herdr.dev/core/install.sh | sh"
		return 0
	fi

	have=$(hc --version 2>/dev/null | awk '{ print $2 }')
	if [ -n "$have" ] && ! at_least "$have" "$HC_MIN"; then
		say ""
		say "rodeo needs hc $HC_MIN or newer, and this hc is $have. Update it with:"
		say "  hc update"
	fi
}

# The case rejects a newline first, since grep -x matches a single line.
is_tag() {
	case $1 in *[!0-9A-Za-z.+-]*) return 1 ;; esac
	printf '%s\n' "$1" | grep -Eqx 'v[0-9]+\.[0-9]+\.[0-9]+[0-9A-Za-z.+-]*'
}

at_least() {
	awk -v have="$1" -v want="$2" 'BEGIN {
		split(have, a, "."); split(want, b, ".")
		for (i = 1; i <= 3; i++) {
			if (a[i] + 0 > b[i] + 0) exit 0
			if (a[i] + 0 < b[i] + 0) exit 1
		}
		exit 0
	}'
}

fetch() {
	if command -v curl >/dev/null 2>&1; then
		curl -fsSL --proto '=https' --tlsv1.2 -o "$2" "$1"
	elif command -v wget >/dev/null 2>&1; then
		wget -qO "$2" "$1"
	else
		die "no curl and no wget"
	fi
}

# The checksum sits beside the archive: it catches a broken download, not a hostile one.
verify() {
	want=$(cut -d' ' -f1 <"$1/$2.sha256")
	if command -v sha256sum >/dev/null 2>&1; then
		got=$(sha256sum "$1/$2" | cut -d' ' -f1)
	elif command -v shasum >/dev/null 2>&1; then
		got=$(shasum -a 256 "$1/$2" | cut -d' ' -f1)
	else
		say "no sha256sum and no shasum; skipping verification"
		return 0
	fi

	[ "$want" = "$got" ] || die "checksum mismatch: expected $want, got $got"
}

say() { printf '%s\n' "$1" >&2; }
die() {
	printf 'install: %s\n' "$1" >&2
	exit 1
}

# Called last so a truncated download cannot half-run.
main "$@"
