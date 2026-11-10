import { readFileSync } from "node:fs";
import { join } from "node:path";

let root = join(import.meta.dirname, "../..");
let read = (file: string) => readFileSync(join(root, file), "utf8");

let manifest = read("Cargo.toml");
let version = /^\s*version\s*=\s*"([^"]+)"/m.exec(manifest.slice(manifest.indexOf("[workspace.package]")))?.[1];

let cli = read("crates/rodeo/src/cli.rs");
let title = /pub const TITLE: &str = "([^"]*)";/.exec(cli)?.[1];
let usage = /pub const USAGE: &str = "([^"]*)";/.exec(cli)?.[1];
if (!version || !title || !usage) throw new Error("data/release.ts: Cargo.toml or crates/rodeo/src/cli.rs changed shape");

export default {
  version,
  hc: "0.1.18",
  title,
  usage: usage
    .split("\n")
    .map((line) => (line ? "  " + line : ""))
    .join("\n"),
};
