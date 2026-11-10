import { readFileSync } from "node:fs";
import { join } from "node:path";

import * as code from "../code";

type Args = { file: string; lang: keyof typeof code.languages; title?: string };

/**
 * A file of the rodeo repository as a highlighted code block, read at build time.
 * @example <bongo:source file="crates/config/src/default.toml" lang="toml" title="rodeo --default-config" />
 */
export default function (args: Args): string {
  let text = readFileSync(join(import.meta.dirname, "../..", args.file), "utf8");
  return code.block(text, args.lang, args.title);
}
