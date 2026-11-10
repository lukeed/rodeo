import { bayer, hash, noise2, pinned, still } from "./pixel";

const P = {
  bg: "#141312",
  panel: "#1c1a18",
  row: "#252320",
  sel: "#3a342e",
  s0: "#2b2825",
  sdim: "#2a2724",
  o0: "#716b63",
  o1: "#8d867c",
  text: "#ebe5db",
  sub0: "#b8b1a6",
  mauve: "#c6a8e6",
  green: "#a7c58b",
  yellow: "#e8c36a",
  red: "#e7756f",
  blue: "#86b6e0",
  teal: "#86cbbb",
  acc: "#e8673c",
};

type Color = keyof typeof P;
type Status = "working" | "done" | "idle" | "blocked";
type Part = [text: string, color: Color, bold?: boolean];
type Mode = "PREFIX" | "NAVIGATE" | null;
type Moment = { typed: number; hc: Status; toast: boolean; mode: Mode; cursor: boolean; spin: string; secs: number };
type Cell = { ch: string; fg: string; bg: string; b: boolean };

const STATE: Record<Status, [string, string]> = {
  working: ["●", P.yellow],
  done: ["●", P.teal],
  idle: ["○", P.green],
  blocked: ["●", P.red],
};

const SHELL: Part[][] = [
  [["~/oss/rodeo ", "o1"], ["main ", "mauve"], ["↑2", "green"]],
  [["❯ ", "acc"], ["cargo test -p rodeo toast", "text"]],
  [["   Compiling ", "green"], ["rodeo v0.1.0 (crates/rodeo)", "text"]],
  [["    Finished ", "green"], ["`test` profile in 4.21s", "text"]],
  [["     Running ", "green"], ["unittests src/main.rs", "text"]],
  [],
  [["running 6 tests", "text"]],
  [["test toast::queue_keeps_fifo_order ... ", "text"], ["ok", "green"]],
  [["test toast::same_pane_replaces_old ... ", "text"], ["ok", "green"]],
  [["test toast::skips_focused_tab ... ", "text"], ["ok", "green"]],
  [["test toast::finished_lasts_5s ... ", "text"], ["ok", "green"]],
  [["test toast::attention_lasts_8s ... ", "text"], ["ok", "green"]],
  [["test toast::mobile_banner_title ... ", "text"], ["ok", "green"]],
  [],
  [["test result: ", "text"], ["ok", "green"], [". 6 passed; 0 failed; 0 ignored", "text"]],
  [],
  [["~/oss/rodeo ", "o1"], ["main ", "mauve"], ["↑2", "green"]],
];

const AGENT: Part[][] = [
  [["● ", "text"], ["Read ", "text", true], ["crates/rodeo/src/toast.rs", "sub0"]],
  [["  └ ", "o0"], ["212 lines", "o0"]],
  [],
  [
    ["● ", "text"],
    ["A toast for the focused tab is dropped, as in Herdr. I'll keep that and add the 1 s delay from ", "text"],
    ["ui.toast.delay_seconds", "mauve"],
    [".", "text"],
  ],
  [],
  [["● ", "green"], ["Update", "text", true], ["(crates/rodeo/src/toast.rs)", "sub0"]],
  [["  └ ", "o0"], ["Updated with 14 additions and 3 removals", "o0"]],
  [],
  [["● ", "green"], ["Bash", "text", true], ["(cargo test -p rodeo toast)", "sub0"]],
  [["  └ ", "o0"], ["test result: ok. 6 passed; 0 failed", "o0"]],
];

const TYPED = "mise run check";
const SPIN = ["·", "✢", "✳", "✶", "✻", "✽", "✻", "✶", "✳", "✢"];
const LINE = new Set("─│┌┐└┘├┤┬┴┼");
const REST = 5;

class Grid {
  cells: Cell[][];

  constructor(
    public cols: number,
    public rows: number,
  ) {
    this.cells = Array.from({ length: rows }, () => Array.from({ length: cols }, () => ({ ch: " ", fg: P.text, bg: P.bg, b: false })));
  }

  put(x: number, y: number, s: string, fg = P.text, bg: string | null = null, b = false): number {
    if (y < 0 || y >= this.rows) return x;
    for (let ch of s) {
      if (x >= 0 && x < this.cols) {
        let c = this.cells[y][x];
        c.ch = ch;
        c.fg = fg;
        c.b = b;
        if (bg) c.bg = bg;
      }
      x++;
    }
    return x;
  }

  clip(x: number, y: number, s: string, max: number, fg?: string, bg?: string | null, b?: boolean): number {
    return this.put(x, y, [...s].slice(0, Math.max(0, max)).join(""), fg, bg, b);
  }

  fill(x: number, y: number, w: number, h: number, bg: string) {
    for (let j = y; j < y + h; j++)
      for (let i = x; i < x + w; i++)
        if (j >= 0 && j < this.rows && i >= 0 && i < this.cols) {
          let c = this.cells[j][i];
          c.bg = bg;
          c.ch = " ";
        }
  }

  shade(x: number, y: number, w: number, bg: string) {
    for (let i = x; i < x + w; i++) if (i < this.cols && y < this.rows) this.cells[y][i].bg = bg;
  }

  box(x: number, y: number, w: number, h: number, fg: string, title?: string, tfg?: string, tb?: boolean) {
    this.put(x, y, "┌" + "─".repeat(w - 2) + "┐", fg);
    for (let j = 1; j < h - 1; j++) {
      this.put(x, y + j, "│", fg);
      this.put(x + w - 1, y + j, "│", fg);
    }
    this.put(x, y + h - 1, "└" + "─".repeat(w - 2) + "┘", fg);
    if (title && w > 4) this.clip(x + 1, y, ` ${title} `, w - 2, tfg, null, tb);
  }
}

function moment(t: number): Moment {
  return {
    typed: t < 1.1 ? 0 : Math.min(TYPED.length, Math.floor((t - 1.1) / 0.085)),
    hc: t >= 2.5 ? "done" : "working",
    toast: t >= 2.5 && t < 9,
    mode: t >= 2.75 && t < 3.3 ? "PREFIX" : t >= 3.3 && t < 7.8 ? "NAVIGATE" : null,
    cursor: still() || Math.floor(t * 1.6) % 2 === 0,
    spin: SPIN[Math.floor(t * 7) % SPIN.length],
    secs: 41 + Math.floor(t),
  };
}

function line(g: Grid, x: number, y: number, parts: Part[], max: number): number {
  let cx = x;
  for (let [s, col, b] of parts) {
    let room = x + max - cx;
    if (room <= 0) break;
    cx = g.clip(cx, y, s, room, P[col], null, !!b);
  }
  return cx;
}

function modeBar(g: Grid, x: number, y: number, w: number, mode: Exclude<Mode, null>) {
  g.fill(x, y, w, 1, P.panel);
  let segs =
    mode === "PREFIX"
      ? [
          ["esc", " cancel  "],
          ["ctrl+b", " send prefix  "],
          ["w", " workspace nav  "],
          ["?", " keybinds"],
        ]
      : [
          ["esc", " back  "],
          ["↑/↓", " workspace  "],
          ["tab", " pane  "],
          ["?", " keybinds"],
        ];
  let cx = g.put(x, y, ` ${mode} `, P.panel, P.acc, true);
  cx = g.put(cx, y, " ", P.o0, P.panel);
  for (let [k, l] of segs) {
    if (cx + [...k].length + [...l.trimEnd()].length > x + w) break;
    cx = g.put(cx, y, k, P.acc, P.panel, true);
    cx = g.clip(cx, y, l, x + w - cx, P.o0, P.panel);
  }
}

type Space = { name: string; branch: string; git: [string, Color][]; st: Status };

function spaces(m: Moment): Space[] {
  return [
    { name: "rodeo", branch: "main", git: [["↑2", "green"]], st: "working" },
    { name: "hc", branch: "main", git: [], st: m.hc },
    { name: "site", branch: "design", git: [["↓1", "red"]], st: "idle" },
    { name: "dotfiles", branch: "main", git: [], st: "idle" },
  ];
}

function shellPane(g: Grid, x: number, y: number, w: number, h: number, m: Moment) {
  let lines = SHELL.slice(Math.max(0, SHELL.length + 1 - h));
  lines.forEach((parts, i) => line(g, x, y + i, parts, w));
  let py = y + lines.length;
  if (py < y + h) {
    let cx = g.put(x, py, "❯ ", P.acc);
    cx = g.put(cx, py, TYPED.slice(0, m.typed), P.text);
    if (m.cursor && cx < x + w) g.put(cx, py, " ", P.bg, P.text);
  }
}

function wrap(parts: Part[], max: number): Part[][] {
  let indent = [...parts[0][0]].length;
  let rows: Part[][] = [[]];
  let col = 0;
  for (let [text, c, b] of parts) {
    for (let tok of text.split(/(\s+)/)) {
      if (!tok) continue;
      let len = [...tok].length;
      if (col + len > max && col > indent && /\S/.test(tok)) {
        rows.push([[" ".repeat(indent), c]]);
        col = indent;
      }
      if (col === indent && rows.length > 1 && !/\S/.test(tok)) continue;
      rows[rows.length - 1].push([tok, c, b]);
      col += len;
    }
  }
  return rows;
}

function agentPane(g: Grid, x: number, y: number, w: number, h: number, m: Moment) {
  let rows = AGENT.flatMap((parts) => (parts.length ? wrap(parts, w) : [[]]));
  rows = rows.slice(Math.max(0, rows.length + 2 - h));
  rows.forEach((parts, i) => line(g, x, y + i, parts, w));
  let py = Math.min(y + h - 1, y + rows.length + 1);
  line(
    g,
    x,
    py,
    [
      [m.spin + " ", "acc"],
      ["Running tests… ", "acc"],
      [`(${m.secs}s · esc to interrupt)`, "o0"],
    ],
    w,
  );
}

function desktop(g: Grid, m: Moment) {
  let { cols, rows } = g;
  let sw = 26;
  g.put(1, 0, "rodeo", P.text, null, true);
  g.put(6, 0, "▍", P.acc);
  g.put(0, 1, "─".repeat(sw - 1), P.sdim);
  let split = Math.max(14, Math.round(rows * 0.56));
  let y = 2;
  spaces(m).forEach((sp, i) => {
    let focused = i === 0;
    let selected = m.mode === "NAVIGATE" && i === 1;
    let bg = selected ? P.sel : focused ? P.row : null;
    if (bg) {
      g.shade(0, y, sw - 1, bg);
      g.shade(0, y + 1, sw - 1, bg);
    }
    let [icon, col] = STATE[sp.st];
    g.put(1, y, icon, col);
    g.put(3, y, sp.name, focused || selected ? P.text : P.sub0, null, focused || selected);
    let cx = g.put(3, y + 1, sp.branch, focused ? P.mauve : P.o0);
    for (let [txt, c] of sp.git) cx = g.put(cx + 1, y + 1, txt, P[c]);
    y += 3;
  });
  g.put(1, split - 1, "new", P.o0);
  g.put(sw - 5, split - 1, "menu", P.o0);
  g.put(0, split, "─".repeat(sw - 1), P.sdim);
  g.put(1, split + 1, "agents", P.o0, null, true);
  g.put(sw - 8, split + 1, "grouped", P.o0, null, true);
  let ay = split + 3;
  let agents: [string, string, Status][] = [
    ["rodeo", "claude", "working"],
    ["hc", "codex", m.hc],
  ];
  for (let [ws, agent, st] of agents) {
    let [icon, col] = STATE[st];
    g.put(1, ay, icon, col);
    g.put(3, ay, ws, P.sub0, null, true);
    g.put(3, ay + 1, agent, P.o0);
    ay += 3;
  }
  g.put(sw - 2, rows - 1, "«", P.o0);
  for (let j = 0; j < rows; j++) g.put(sw - 1, j, "│", P.sdim);

  let mx = sw;
  let mw = cols - sw;
  g.fill(mx, 0, mw, 1, P.panel);
  let tx = mx;
  let tabs: [string, boolean, boolean][] = [
    ["code", true, true],
    ["tests", false, true],
    ["3", false, false],
  ];
  for (let [label, focused, custom] of tabs) {
    let w = Math.max(8, label.length + 4);
    let pad = Math.floor((w - label.length) / 2);
    let bg = focused ? P.acc : P.s0;
    let fg = focused ? P.panel : custom ? P.o1 : P.o0;
    g.fill(tx, 0, w, 1, bg);
    g.put(tx + pad, 0, label, fg, bg, focused && custom);
    tx += w + 1;
  }
  g.put(tx, 0, " + ", P.o1, P.panel);
  let py = 1;
  let ph = rows - 1;
  let lw = Math.floor(mw * 0.5);
  let rw = mw - lw;
  g.box(mx, py, lw, ph, P.acc, "shell", P.acc, true);
  g.box(mx + lw, py, rw, ph, P.o0, "claude", P.o0, false);
  shellPane(g, mx + 2, py + 1, lw - 4, ph - 2, m);
  agentPane(g, mx + lw + 2, py + 1, rw - 4, ph - 2, m);
  if (m.mode) modeBar(g, mx, rows - 1, mw, m.mode);
  if (m.toast) {
    let title = "codex finished";
    let body = "hc · 2";
    let w = Math.max(title.length, body.length) + 6;
    let h = 4;
    let x = cols - w - 1;
    let ty = rows - h - (m.mode ? 2 : 1);
    g.fill(x, ty, w, h, P.panel);
    g.box(x, ty, w, h, P.o0);
    g.put(x + 2, ty + 1, "●", P.blue, P.panel);
    g.put(x + 4, ty + 1, title, P.text, P.panel, true);
    g.put(x + 2, ty + 2, body, P.o0, P.panel);
  }
}

function mobile(g: Grid, m: Moment) {
  let { cols, rows } = g;
  g.fill(0, 0, cols, 2, P.panel);
  let [icon, col] = STATE.working;
  g.put(1, 0, icon, col, P.panel);
  g.put(3, 0, "rodeo", P.text, P.panel, true);
  let sx = cols - 10;
  let tab = "tab code · 1/3";
  g.put(sx - tab.length - 1, 0, tab, P.o1, P.panel);
  g.fill(sx, 0, 10, 2, P.s0);
  g.put(sx, 0, "│", P.sdim, P.s0);
  g.put(sx, 1, "│", P.sdim, P.s0);
  g.put(sx + 3, 0, "switch", P.text, P.s0);
  if (m.hc === "done") {
    let cx = g.put(1, 1, "1 done", P.blue, P.panel, true);
    g.put(cx, 1, " · 1 working", P.o1, P.panel);
  } else g.put(1, 1, "2 working", P.yellow, P.panel, true);
  let top = 2;
  let h = rows - top;
  let h1 = Math.floor(h * 0.55);
  let h2 = h - h1;
  g.box(0, top, cols, h1, P.acc, "shell", P.acc, true);
  g.box(0, top + h1, cols, h2, P.o0, "claude", P.o0, false);
  shellPane(g, 2, top + 1, cols - 4, h1 - 2, m);
  agentPane(g, 2, top + h1 + 1, cols - 4, h2 - 2, m);
  let bottom = rows - 1;
  if (m.mode) {
    modeBar(g, 0, bottom, cols, "PREFIX");
    bottom--;
  }
  if (m.toast) {
    g.fill(0, bottom, cols, 1, P.s0);
    let cx = g.put(1, bottom, "●", P.blue, P.s0);
    cx = g.put(cx + 1, bottom, "codex done", P.text, P.s0, true);
    g.put(cx, bottom, " · hc · 2", P.o1, P.s0);
  }
}

function render(ctx: CanvasRenderingContext2D, g: Grid, cw: number, ch: number, font: string, bold: string) {
  ctx.fillStyle = P.bg;
  ctx.fillRect(0, 0, g.cols * cw, g.rows * ch);
  for (let y = 0; y < g.rows; y++)
    for (let x = 0; x < g.cols; x++) {
      let c = g.cells[y][x];
      if (c.bg !== P.bg) {
        ctx.fillStyle = c.bg;
        ctx.fillRect(Math.floor(x * cw), y * ch, Math.ceil(cw) + 1, ch);
      }
    }
  ctx.textBaseline = "middle";
  for (let y = 0; y < g.rows; y++)
    for (let x = 0; x < g.cols; x++) {
      let c = g.cells[y][x];
      if (c.ch === " ") continue;
      let px = x * cw;
      let py = y * ch;
      let mx = Math.floor(px + cw / 2);
      let my = Math.floor(py + ch / 2);
      ctx.fillStyle = c.fg;
      if (LINE.has(c.ch)) {
        let k = c.ch;
        if ("─┐┘┤┬┴┼".includes(k)) ctx.fillRect(Math.floor(px), my, mx - Math.floor(px) + 1, 1);
        if ("─┌└├┬┴┼".includes(k)) ctx.fillRect(mx, my, Math.ceil(px + cw) - mx, 1);
        if ("│└┘├┤┴┼".includes(k)) ctx.fillRect(mx, py, 1, my - py + 1);
        if ("│┌┐├┤┬┼".includes(k)) ctx.fillRect(mx, my, 1, py + ch - my);
        continue;
      }
      if (c.ch === "▍") {
        ctx.fillRect(Math.round(px), py + 1, Math.round(cw * 0.42), ch - 2);
        continue;
      }
      if (c.ch === "●" || c.ch === "○") {
        ctx.beginPath();
        ctx.arc(px + cw / 2, py + ch / 2, cw * 0.36, 0, Math.PI * 2);
        if (c.ch === "●") ctx.fill();
        else {
          ctx.strokeStyle = c.fg;
          ctx.lineWidth = 1.2;
          ctx.stroke();
        }
        continue;
      }
      ctx.font = c.b ? bold : font;
      ctx.fillText(c.ch, px, py + ch / 2 + 1);
    }
}

function reveal(canvas: HTMLCanvasElement, width: number, height: number, bg: string) {
  let S = 3;
  let W = Math.ceil(width / S);
  let H = Math.ceil(height / S);
  canvas.width = W;
  canvas.height = H;
  canvas.style.width = W * S + "px";
  canvas.style.height = H * S + "px";
  let ctx = canvas.getContext("2d")!;
  let img = ctx.createImageData(W, H);
  let base = [1, 3, 5].map((i) => parseInt(bg.slice(i, i + 2), 16));
  let DUST = [
    [255, 241, 220],
    [244, 160, 110],
    [232, 103, 60],
    [140, 70, 46],
  ];
  let th = new Float32Array(W * H);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      let field = noise2(x * 0.07, y * 0.11, 3) * 0.6 + noise2(x * 0.2, y * 0.25, 4) * 0.4;
      th[y * W + x] = 0.55 * field + 0.3 * bayer(x, y) + 0.15 * (x / W);
    }
  let dur = 1.1;
  let t0 = performance.now();
  let step = () => {
    let t = (performance.now() - t0) / 1000;
    let p = Math.min(1.08, (t / dur) * 1.08);
    let d = img.data;
    for (let i = 0; i < W * H; i++) {
      let v = th[i];
      let o = i * 4;
      if (p > v + 0.08) {
        d[o + 3] = 0;
        continue;
      }
      if (p > v) {
        let c = DUST[Math.min(3, Math.floor(((p - v) / 0.08) * 4))];
        d[o] = c[0];
        d[o + 1] = c[1];
        d[o + 2] = c[2];
        d[o + 3] = hash(i, Math.floor(t * 20)) > 0.5 ? 255 : 150;
        continue;
      }
      d[o] = base[0];
      d[o + 1] = base[1];
      d[o + 2] = base[2];
      d[o + 3] = 255;
    }
    ctx.putImageData(img, 0, 0);
    if (p < 1.08) requestAnimationFrame(step);
    else canvas.remove();
  };
  step();
}

async function screen(box: HTMLElement) {
  let canvas = box.querySelector<HTMLCanvasElement>("canvas.screen")!;
  let dust = box.querySelector<HTMLCanvasElement>("canvas.dust");
  let ctx = canvas.getContext("2d")!;
  await document.fonts.load("13px 'Geist Mono'");
  await document.fonts.load("600 13px 'Geist Mono'");
  let t0: number | null = null;
  let layout = { cols: 0, rows: 0, cw: 0, ch: 0, font: "", bold: "", dpr: 1, phone: false };

  let setup = () => {
    let w = box.clientWidth;
    let phone = w < 560;
    let fs = phone ? 11 : w < 1000 ? 12 : 13;
    let font = `400 ${fs}px "Geist Mono"`;
    ctx.font = font;
    let cols = Math.floor(w / ctx.measureText("M").width);
    let cw = w / cols;
    let ch = Math.round(fs * 1.38);
    let rows = 30;
    let dpr = window.devicePixelRatio || 1;
    canvas.width = Math.round(cols * cw * dpr);
    canvas.height = rows * ch * dpr;
    canvas.style.width = cols * cw + "px";
    canvas.style.height = rows * ch + "px";
    box.style.setProperty("--tui-h", rows * ch + "px");
    layout = { cols, rows, cw, ch, font, bold: `650 ${fs}px "Geist Mono"`, dpr, phone };
  };

  let draw = (t: number) => {
    let { cols, rows, cw, ch, font, bold, dpr, phone } = layout;
    let g = new Grid(cols, rows);
    (phone || cols <= 64 ? mobile : desktop)(g, moment(t));
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    render(ctx, g, cw, ch, font, bold);
  };

  let now = () => (still() ? (pinned ?? REST) : t0 == null ? 0 : (performance.now() - t0) / 1000);

  setup();
  new ResizeObserver(() => {
    setup();
    draw(now());
  }).observe(box);

  if (still()) {
    dust?.remove();
    draw(now());
    return;
  }

  draw(0);
  let visible = false;
  let loop = () => {
    if (!visible) return;
    draw(now());
    setTimeout(() => requestAnimationFrame(loop), 1000 / 15);
  };
  new IntersectionObserver(([entry]) => {
    let was = visible;
    visible = entry.isIntersecting;
    if (visible && t0 == null) {
      t0 = performance.now();
      if (dust) reveal(dust, box.clientWidth, layout.rows * layout.ch, "#0f0e0d");
      loop();
    } else if (visible && !was) loop();
  }).observe(box);
}

/**
 * Draw the live rodeo screen into every `.tui` box.
 * @example screens()
 */
export function screens(): void {
  for (let box of document.querySelectorAll<HTMLElement>(".tui")) void screen(box);
}
