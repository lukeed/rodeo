import { markdown } from "@bongo/markdown";
import { twinkleplop } from "@bongo/markdown/twinkleplop";

import * as code from "./code";
import * as utils from "./utils";

import type { Config } from "bongo";

export default {
  $site: {
    lang: "en",
    title: "rodeo",
    url: "https://rodeo.lukeed.com/",
    description:
      "rodeo is a keyboard-first terminal UI for herdr-core (hc). It shares spaces, tabs and panes with the Herdr desktop app.",
  },
  $utils: { ...utils },
  $hooks: {
    transform: code.transform,
  },
  plugins: [
    markdown({
      headingIds: true,
      highlighter: twinkleplop({ languages: code.languages }),
      fence: code.fence,
    }),
  ],
} satisfies Config;
