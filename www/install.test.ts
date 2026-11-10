import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const SCRIPT = join(import.meta.dirname, "install.sh");
const TARGET = `${process.arch === "arm64" ? "aarch64" : "x86_64"}-${process.platform === "darwin" ? "apple-darwin" : "unknown-linux-musl"}`;
const ARCHIVE = `rodeo-${TARGET}.tar.gz`;

let root: string;
let release: string;

function executable(file: string, body: string) {
  writeFileSync(file, `#!/bin/sh\n${body}\n`);
  chmodSync(file, 0o755);
}

function sh(cmd: string[], cwd?: string) {
  let done = Bun.spawnSync(cmd, { cwd });
  if (done.exitCode !== 0) throw new Error(done.stderr.toString());
}

beforeAll(() => {
  root = mkdtempSync(join(tmpdir(), "rodeo-install-"));
  release = join(root, "release");
  let stage = join(root, "stage");
  mkdirSync(release);
  mkdirSync(stage);
  executable(join(stage, "rodeo"), 'echo "rodeo 9.9.9"');
  sh(["tar", "-czf", join(release, ARCHIVE), "-C", stage, "rodeo"], root);
  let sum = new Bun.CryptoHasher("sha256").update(readFileSync(join(release, ARCHIVE))).digest("hex");
  writeFileSync(join(release, `${ARCHIVE}.sha256`), `${sum}  ${ARCHIVE}\n`);
  writeFileSync(join(release, "latest"), "v0.1.0\n");
});

afterAll(() => rmSync(root, { recursive: true, force: true }));

function run(opts: { hc?: string; version?: string; files?: Record<string, string>; drop?: string[] } = {}) {
  let home = mkdtempSync(join(root, "home-"));
  let bin = join(home, "stubs");
  let files = join(home, "files");
  mkdirSync(bin);
  mkdirSync(files);
  sh(["cp", "-R", `${release}/.`, files]);
  for (let [name, body] of Object.entries(opts.files ?? {})) writeFileSync(join(files, name), body);
  for (let name of opts.drop ?? []) rmSync(join(files, name));
  executable(
    join(bin, "curl"),
    `out=""; url=""
while [ $# -gt 0 ]; do case $1 in -o) out=$2; shift 2 ;; *) url=$1; shift ;; esac; done
echo "$url" >> "${home}/urls"
[ -f "${files}/$(basename "$url")" ] || exit 22
cp "${files}/$(basename "$url")" "$out"`,
  );
  if (opts.hc) executable(join(bin, "hc"), `echo "hc ${opts.hc}"`);
  let env: Record<string, string> = { HOME: home, PATH: `${bin}:/usr/bin:/bin`, RODEO_INSTALL: join(home, "bin") };
  if (opts.version) env.RODEO_VERSION = opts.version;
  let done = Bun.spawnSync(["sh", SCRIPT], { env });
  let urls = existsSync(join(home, "urls")) ? readFileSync(join(home, "urls"), "utf8").trim().split("\n") : [];
  return { code: done.exitCode, err: done.stderr.toString(), urls, binary: join(home, "bin", "rodeo") };
}

describe("install.sh", () => {
  test("installs the latest release for this machine and checks its sha256", () => {
    let out = run({ hc: "0.1.18" });
    expect(out.code).toBe(0);
    expect(out.urls).toEqual([
      "https://rodeo.lukeed.com/download/latest",
      `https://rodeo.lukeed.com/download/v0.1.0/${ARCHIVE}`,
      `https://rodeo.lukeed.com/download/v0.1.0/${ARCHIVE}.sha256`,
    ]);
    expect(out.err).toContain(`downloading rodeo v0.1.0 for ${TARGET}`);
    expect(out.err).toContain(`installed rodeo 9.9.9 to ${out.binary}`);
    expect(out.err).toContain("is not on your PATH");
    expect(out.err).not.toContain("needs hc");
    expect(existsSync(out.binary)).toBe(true);
  });

  test("pins a release with RODEO_VERSION", () => {
    let out = run({ hc: "0.1.18", version: "v0.0.9" });
    expect(out.code).toBe(0);
    expect(out.urls).toEqual([
      `https://rodeo.lukeed.com/download/v0.0.9/${ARCHIVE}`,
      `https://rodeo.lukeed.com/download/v0.0.9/${ARCHIVE}.sha256`,
    ]);
  });

  test("refuses a RODEO_VERSION that is not a release tag", () => {
    for (let version of ["0.1.0", "latest", "v0.1.0/../x", "v0.1.0 x"]) {
      let out = run({ hc: "0.1.18", version });
      expect(out.code).toBe(1);
      expect(out.err).toContain(`"${version}" is not a release tag, such as v0.1.0`);
      expect(out.urls).toEqual([]);
    }
  });

  test("stops when it cannot find the latest release", () => {
    let out = run({ hc: "0.1.18", drop: ["latest"] });
    expect(out.code).toBe(1);
    expect(out.err).toContain("could not find the latest release");
    expect(out.urls).toEqual(["https://rodeo.lukeed.com/download/latest"]);
  });

  test("stops when the latest release is not a tag", () => {
    let out = run({ hc: "0.1.18", files: { latest: "<html>oops</html>\n" } });
    expect(out.code).toBe(1);
    expect(out.err).toContain('"<html>oops</html>" is not a release tag');
    expect(out.urls).toEqual(["https://rodeo.lukeed.com/download/latest"]);
    expect(run({ hc: "0.1.18", files: { latest: "v0.1.0\n<html>\n" } }).code).toBe(1);
  });

  test("refuses an archive that does not match its checksum", () => {
    let out = run({ hc: "0.1.18", files: { [`${ARCHIVE}.sha256`]: `${"0".repeat(64)}  ${ARCHIVE}\n` } });
    expect(out.code).toBe(1);
    expect(out.err).toContain("checksum mismatch");
    expect(existsSync(out.binary)).toBe(false);
  });

  test("refuses an archive with no checksum", () => {
    let out = run({ hc: "0.1.18", drop: [`${ARCHIVE}.sha256`] });
    expect(out.code).toBe(1);
    expect(out.err).toContain(`could not download ${ARCHIVE}.sha256`);
    expect(existsSync(out.binary)).toBe(false);
  });

  test("says how to build from source when there is no release", () => {
    let out = run({ hc: "0.1.18", drop: [ARCHIVE] });
    expect(out.code).toBe(1);
    expect(out.err).toContain(`could not download ${ARCHIVE} for v0.1.0`);
    expect(out.err).toContain("https://rodeo.lukeed.com/docs/install/#from-source");
  });

  test("says how to get hc when it is not on the PATH", () => {
    let out = run();
    expect(out.code).toBe(0);
    expect(out.err).toContain("rodeo needs hc (herdr-core) 0.1.18 or newer on your PATH");
    expect(out.err).toContain("curl -fsSL https://releases.herdr.dev/core/install.sh | sh");
  });

  test("says to update an hc that is too old", () => {
    let out = run({ hc: "0.1.9" });
    expect(out.err).toContain("rodeo needs hc 0.1.18 or newer, and this hc is 0.1.9");
    expect(run({ hc: "0.2.0" }).err).not.toContain("needs hc");
  });
});
