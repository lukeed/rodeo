import { Buf, bayer, hash, mount, noise1, rgb, streak } from "./pixel";

import type { RGB } from "./pixel";

type Rect = { x0: number; y0: number; x1: number; y1: number };
type Hill = { body: RGB; rim: RGB; lit: RGB };
type Star = { x: number; y: number; col: RGB; kind: Glyph; tw: boolean };
type Glyph = "dot" | "dot2" | "plus" | "spark";
type Pose = "stand" | "graze" | "hop" | "look" | "lamb";
type Ram = { kind: Pose; x: number; feet: number; flip: boolean; seed: number; back: boolean; hops?: boolean };
type Fire = { x: number; y: number; small: boolean };
type State = { base: Buf; stars: Star[]; fire?: Fire; rams?: Ram[]; front?: Buf };
type Theme = {
  sky: RGB[];
  bg: RGB;
  stars: RGB[];
  far: { body: RGB; rim: RGB };
  hills: Hill[];
  ink: RGB;
  fenceRim: RGB;
  fenceLit: RGB;
  pool: RGB[];
  hat: Record<string, RGB>;
};

const ramp = (list: string[]) => list.map(rgb);
const scale = (c: RGB, k: number): RGB => [c[0] * k, c[1] * k, c[2] * k];

const INK: Theme = {
  sky: ramp(["#0f0e0d", "#100f0e", "#121110", "#151311", "#191613", "#1e1915", "#251e18"]),
  bg: rgb("#0f0e0d"),
  stars: ramp(["#efe4d2", "#cfc3b0", "#efe4d2", "#ffb98a"]),
  far: { body: rgb("#1b1714"), rim: rgb("#2e2520") },
  hills: [
    { body: rgb("#181512"), rim: rgb("#2b231d"), lit: rgb("#3a2e25") },
    { body: rgb("#141210"), rim: rgb("#27201a"), lit: rgb("#3a2c22") },
  ],
  ink: rgb("#090807"),
  fenceRim: rgb("#2b231d"),
  fenceLit: rgb("#3d2f25"),
  pool: ramp(["#3a2016", "#2a1912", "#1d1512"]),
  hat: { L: rgb("#ffe8cf"), M: rgb("#b7aa9a"), D: rgb("#7a6c5e"), R: rgb("#ff8a52"), r: rgb("#a8462a") },
};

const RAM: Record<string, RGB> = {
  W: rgb("#efe4d0"),
  w: rgb("#cfc0a8"),
  s: rgb("#86808f"),
  F: rgb("#3a2c26"),
  f: rgb("#221a16"),
  e: rgb("#e8d9b8"),
  E: rgb("#5a463c"),
  H: rgb("#e0b070"),
  h: rgb("#a8743f"),
  d: rgb("#5e3c22"),
  L: rgb("#3a2c26"),
  l: rgb("#1a1411"),
};

const DUST = [rgb("#bfae96"), rgb("#6a5a4c")];
const FLAME = ramp(["#fff1c9", "#ffd27a", "#ff9a3c", "#e8673c", "#a8462a"]);
const LOG = rgb("#2a1430");
const LOGLIT = rgb("#7a3a2a");
const WARM: RGB = [255, 140, 60];

const HAT = ["....CC..CC....", "....CCCCCC....", "C...CCCCCC...C", "CC..BBBBBB..CC", ".CCCCCCCCCCCC."];
const HAT2 = [
  "........MMMM....LLLL........",
  "........MMMMMLLLLLLL........",
  "........MMMMMLLLLLLL........",
  "........MMMMMMLLLLLL........",
  "MM......MMMMMMLLLLLL......LL",
  "MM......MMMMMMLLLLLL......LL",
  "MMMM....rrrrrRRRRRRR....LLLL",
  "DMMM....rrrrrrRRRRRR....LLLL",
  "..MMMMMMMMMMMMLLLLLLLLLLLL..",
  "..DDDMMMMMMMMMMMMMMMLLLLLL..",
];

const BODY = [
  "......wWW...wWW...",
  "....wWWWWWwwWWWWw.",
  "...wWWWWWWWWWWWWWw",
  "..wWWWWWWWWWWWWWWW",
  ".wWWWWWWWWWWWWWWWW",
  "swWWWWWWWWWWWWWWWw",
  "swwWWWWWWWWWWWWWww",
  "sswwWWWWWWWWWWWwws",
  "ssswwwwwwwwwwwwwss",
  ".sssswwwwwwwwwwss.",
  "..sssssssssssssss.",
];
const LEGS = {
  stand: ["...LL.LL....LL.LL.", "...LL.LL....LL.LL.", "...ll.ll....ll.ll."],
  hop: ["..LL..LL...LL..LL.", ".ll....ll.ll....ll", ".................."],
};
const HEAD = ["..FFFF...", ".FFFFFF..", "EFFFFeFF.", "EFFFFFFFF", ".FFFFFFFF", "..FFFFFf.", "...ffff.."];
const HORN = [
  "...HHHHh..",
  "..HhhhhhH.",
  ".Hhd...dhH",
  "Hhd.HHh.dh",
  "Hd.Hhdhd.d",
  "hd.hd.hd..",
  "hd..ddd...",
  ".hd......",
  "..ddd....",
];
const LAMB = [
  "................",
  "....wWW.wW......",
  "..wWWWWWWWWw.hH.",
  ".wWWWWWWWWWWhdFF",
  "swWWWWWWWWWwhFeF",
  "sswwWWWWWWwwsFFF",
  ".ssswwwwwwwss.f.",
  "..sssssssssss...",
  "..LL.L...L.LL...",
  "..ll.l...l.ll...",
];

function put(grid: string[][], art: string[], ox: number, oy: number) {
  art.forEach((row, j) =>
    [...row].forEach((ch, i) => {
      if (ch === "." || !grid[oy + j] || ox + i < 0 || ox + i >= grid[0].length) return;
      grid[oy + j][ox + i] = ch;
    }),
  );
}

function build(pose: Pose): string[] {
  let g = Array.from({ length: 17 }, () => Array<string>(28).fill("."));
  put(g, BODY, 0, 3);
  put(g, LEGS[pose === "hop" ? "hop" : "stand"], 0, 14);
  let hy = pose === "graze" ? 8 : pose === "look" ? 1 : 3;
  if (pose === "look") put(g, ["ww", "Www", "WWw", "wWw"], 17, 3);
  if (pose === "graze") put(g, ["w..", "Ww.", "wWw", ".ww"], 17, 7);
  put(g, HEAD, 19, hy + 1);
  put(g, HORN, 14, hy - 2);
  return g.map((r) => r.join(""));
}

const RAMS: Record<Pose, string[]> = {
  stand: build("stand"),
  graze: build("graze"),
  hop: build("hop"),
  look: build("look"),
  lamb: LAMB,
};

function sky(b: Buf, w: number, y0: number, y1: number, list: RGB[]) {
  for (let y = y0; y < y1; y++) {
    let t = Math.max(0, Math.min(1, (y - y0) / Math.max(1, y1 - y0))) * (list.length - 1);
    let i = Math.min(list.length - 2, Math.floor(t));
    for (let x = 0; x < w; x++) b.set(x, y, streak(x, y, t - i) ? list[i + 1] : list[i]);
  }
}

function glyph(b: Buf, x: number, y: number, col: RGB, kind: Glyph) {
  b.set(x, y, col);
  if (kind === "dot2") {
    b.set(x + 1, y, col);
    b.set(x, y + 1, col);
    b.set(x + 1, y + 1, col);
  }
  if (kind === "plus" || kind === "spark") {
    let dim = scale(col, 0.55);
    b.set(x - 1, y, dim);
    b.set(x + 1, y, dim);
    b.set(x, y - 1, dim);
    b.set(x, y + 1, dim);
    if (kind === "spark") {
      let d2 = scale(dim, 0.7);
      b.set(x, y - 2, d2);
      b.set(x, y + 2, d2);
    }
  }
}

function stars(b: Buf, w: number, y0: number, y1: number, density: number, avoid: Rect[], seed: number): Star[] {
  let list: Star[] = [];
  let cell = 11;
  let twinkles = 0;
  for (let gy = Math.floor(y0 / cell); gy * cell < y1; gy++)
    for (let gx = 0; gx * cell < w; gx++) {
      if (hash(gx, gy, seed) > density) continue;
      let x = gx * cell + Math.floor(hash(gx, gy, seed + 1) * cell);
      let y = gy * cell + Math.floor(hash(gx, gy, seed + 2) * cell);
      if (y < y0 || y >= y1) continue;
      if (avoid.some((r) => x >= r.x0 - 6 && x <= r.x1 + 6 && y >= r.y0 - 4 && y <= r.y1 + 4)) continue;
      let r = hash(gx, gy, seed + 3);
      let kind: Glyph = r > 0.97 ? "spark" : r > 0.92 ? "plus" : r > 0.82 ? "dot2" : "dot";
      let col = INK.stars[Math.floor(hash(gx, gy, seed + 4) * INK.stars.length)];
      let fade = 1 - ((y - y0) / Math.max(1, y1 - y0)) * 0.45;
      if (kind === "dot") col = scale(col, (0.5 + hash(gx, gy, seed + 5) * 0.5) * fade);
      let tw = kind === "plus" && twinkles < 4;
      if (tw) twinkles++;
      list.push({ x, y, col, kind, tw });
      if (!tw) glyph(b, x, y, col, kind);
    }
  return list;
}

function twinkle(b: Buf, list: Star[], t: number) {
  list.forEach((s, i) => {
    if (!s.tw) return;
    let period = 3.5 + hash(i, 6) * 3.5;
    let ph = ((t + hash(i, 7) * period) % period) / period;
    let level = ph < 0.08 ? 1 : ph < 0.18 ? 0.8 : ph < 0.6 ? 0.6 : 0.8;
    let col = scale(s.col, level);
    glyph(b, s.x, s.y, col, level === 1 ? "spark" : level > 0.7 ? "plus" : "dot");
  });
}

function mesa(b: Buf, w: number, x0: number, x1: number, top: number, base: number, seed: number) {
  let hgt = base - top;
  let talus = Math.max(5, Math.min(Math.round(hgt * 0.45), Math.round((x1 - x0) * 0.2)));
  let shoulder = top + Math.round(hgt * 0.62);
  for (let x = Math.max(0, x0); x <= Math.min(w - 1, x1); x++) {
    let dl = x - x0;
    let dr = x1 - x;
    let d = x1 >= w ? dl : x0 < 0 ? dr : Math.min(dl, dr);
    let y = d < talus ? Math.round(base - (d / talus) * (base - shoulder)) : d < talus + 4 ? top + 5 : top;
    y += Math.round((noise1(x * 0.35, seed) - 0.5) * 2);
    for (let yy = y; yy < base; yy++) b.set(x, yy, INK.far.body);
    b.set(x, y, INK.far.rim);
  }
}

function hills(b: Buf, w: number, h: number, layers: { base: number; amp: number }[]) {
  layers.forEach((layer, li) => {
    let hill = INK.hills[li];
    let top: number[] = [];
    let s = 0.018 + li * 0.006;
    for (let x = 0; x < w; x++)
      top.push(Math.round(layer.base - layer.amp * (noise1(x * s, 11 + li * 5) * 0.75 + noise1(x * s * 3.1, 29 + li) * 0.25)));
    for (let x = 0; x < w; x++) {
      let y0 = top[x];
      let slope = (top[Math.min(w - 1, x + 1)] - top[Math.max(0, x - 1)]) / 2;
      for (let y = y0; y < h; y++) b.set(x, y, hill.body);
      b.set(x, y0, slope > 0.15 ? hill.lit : hill.rim);
      if (slope > 0.35) b.set(x, y0 + 1, hill.rim);
      let tuft = hash(x, li, 77);
      if (tuft > 0.84) b.set(x, y0 - 1, hill.rim);
      if (tuft > 0.96) b.set(x, y0 - 2, hill.rim);
    }
  });
}

function hat(b: Buf, sprite: string[], hx: number, hy: number, small: boolean) {
  sprite.forEach((row, j) =>
    [...row].forEach((ch, i) => {
      if (ch === ".") return;
      let key = ch;
      if (small) key = ch === "B" ? (i >= 7 ? "R" : "r") : i >= 7 || j === 4 ? "L" : "M";
      b.set(hx + i, hy + j, INK.hat[key]);
    }),
  );
}

function pool(b: Buf, fx: number, fy: number, rx: number) {
  for (let y = fy - 10; y < fy + 12; y++)
    for (let x = fx - rx; x < fx + rx; x++) {
      let d = Math.hypot((x - fx) / 2.6, (y - fy) * 1.6) / 17;
      let i = Math.floor(d * 3 + bayer(x, y) * 0.9);
      if (i >= 0 && i < 3) b.set(x, y, INK.pool[i]);
    }
}

function logs(b: Buf, fx: number, fy: number, lw: number) {
  for (let i = -lw; i <= lw; i++) {
    b.set(fx + i, fy - Math.round(i * 0.2), i % 3 === 0 ? LOGLIT : LOG);
    b.set(fx + i, fy - 1 + Math.round(-i * 0.2), LOG);
    b.set(fx + i, fy + 1, LOG);
  }
}

function fire(b: Buf, f: Fire, t: number) {
  let k = Math.floor(t * 8);
  let fx = f.x;
  let fy = f.y - 1;
  let w = f.small ? 4 : 6;
  let hmax = f.small ? 10 : 15;
  let r = (f.small ? 18 : 26) * (0.8 + 0.2 * hash(k, 77));
  for (let y = Math.floor(fy - r); y < fy + r * 0.5; y++)
    for (let x = Math.floor(fx - r * 1.4); x < fx + r * 1.4; x++) {
      let d = Math.hypot((x - fx) / 1.4, y - fy + 4) / r;
      if (d >= 1) continue;
      let step = Math.floor((1 - d) * 3 + bayer(x, y) * 0.8) / 3;
      if (step > 0) b.add(x, y, WARM, step * 0.15);
    }
  for (let dx = -w; dx <= w; dx++) {
    let n = 1 - Math.abs(dx) / (w + 1);
    let h = Math.round(hmax * Math.pow(n, 1.3) * (0.7 + 0.45 * hash(k, dx + 20, 5)));
    let lean = Math.round((hash(k, 3) - 0.5) * 2);
    for (let j = 0; j < h; j++) {
      let rel = j / Math.max(1, h);
      let core = Math.abs(dx) <= w * 0.35 && rel < 0.45;
      let idx = core ? (rel < 0.2 && Math.abs(dx) <= 1 ? 0 : 1) : rel < 0.55 ? 2 : rel < 0.85 ? 3 : 4;
      if (idx === 4 && bayer(fx + dx, fy - j) > 0.5) continue;
      b.set(fx + dx + (j > h * 0.6 ? lean : 0), fy - j, FLAME[idx]);
    }
  }
  for (let i = 0; i < (f.small ? 3 : 6); i++) {
    let period = 1.6 + hash(i, 1) * 1.4;
    let ph = (t + hash(i, 2) * period) % period;
    let life = period * 0.8;
    if (ph > life) continue;
    let age = ph / life;
    let x = fx + (hash(i, 3) - 0.5) * 6 + Math.sin(ph * 3 + i) * 2;
    let y = fy - hmax * 0.6 - ph * (14 + hash(i, 4) * 10);
    if (bayer(Math.round(x), Math.round(y)) < age) continue;
    b.set(x, y, FLAME[Math.min(4, 1 + Math.floor(age * 4))]);
  }
}

function dissolve(b: Buf, w: number, from: number, to: number, up = false) {
  for (let y = from; y < to; y++) {
    let t = up ? 1 - (y - from) / (to - from) : (y - from + 1) / (to - from);
    for (let x = 0; x < w; x++) if (streak(x, y, t, 2, 6)) b.set(x, y, INK.bg);
  }
}

function corral(b: Buf, w: number, h: number, o: { x0: number; x1: number; yb: number; yf: number; gap: number; small: boolean }): Buf {
  let { x0, x1, yb, yf, gap, small } = o;
  let front = new Buf(w, h);
  let postF = small ? 10 : 13;
  let postB = small ? 7 : 10;
  for (let x = x0; x <= x1; x++) {
    b.set(x, yb - Math.round(postB * 0.35), INK.fenceRim);
    b.set(x, yb - Math.round(postB * 0.75), INK.fenceRim);
  }
  for (let x = x0; x <= x1; x += gap - 6)
    for (let y = yb - postB; y <= yb; y++) {
      b.set(x, y, INK.ink);
      b.set(x + 1, y, INK.fenceRim);
    }
  for (let k = 0; k <= yf - yb; k++) {
    let y = yb + k;
    for (let x of [x0 - Math.round(k * 0.35), x1 + Math.round(k * 0.35)]) {
      front.set(x, y - Math.round(postF * 0.3), INK.fenceRim);
      front.set(x, y - Math.round(postF * 0.7), INK.fenceRim);
    }
  }
  let fx0 = x0 - Math.round((yf - yb) * 0.35);
  let fx1 = x1 + Math.round((yf - yb) * 0.35);
  for (let r of small ? [0.32, 0.68] : [0.28, 0.66])
    for (let x = fx0; x <= fx1; x++) {
      let y = yf - Math.round(postF * (1 - r));
      front.set(x, y, INK.fenceRim);
      if (!small) front.set(x, y + 1, INK.ink);
    }
  let pw = small ? 3 : 4;
  for (let x = fx0; x <= fx1 - pw; x += gap) {
    let top = yf - postF - Math.floor(hash(x, 7) * 2);
    for (let y = top; y <= yf; y++) for (let i = 0; i < pw; i++) front.set(x + i, y, i === pw - 1 ? INK.fenceRim : INK.ink);
    for (let i = 0; i < pw; i++) front.set(x + i, top, i >= pw / 2 ? INK.fenceLit : INK.fenceRim);
  }
  let gw = small ? 4 : 8;
  let gtop = yf - postF - (small ? 3 : 6);
  for (let y = gtop; y <= yf; y++)
    for (let i = 0; i < gw; i++) front.set(fx0 + i, y, i >= gw - (small ? 1 : 2) ? INK.fenceRim : INK.ink);
  let sprite = small ? HAT : HAT2;
  hat(front, sprite, fx0 + gw / 2 - Math.floor(sprite[0].length / 2), gtop - sprite.length, small);
  return front;
}

function herd(x0: number, x1: number, yb: number, yf: number, count: number, seed: number): Ram[] {
  let order: Pose[] = ["stand", "graze", "lamb", "look", "graze", "lamb", "stand"];
  let kinds = order.slice(0, count);
  let widths = kinds.map((k) => RAMS[k][0].length);
  let span = x1 - x0 - 16;
  let sum = widths.reduce((a, b) => a + b, 0);
  let gap = Math.max(-8, Math.min(10, (span - sum) / Math.max(1, count - 1)));
  let cursor = x0 + 12 + Math.max(0, (span - sum - gap * (count - 1)) / 2);
  let rams: Ram[] = kinds.map((kind, i) => {
    let x = Math.round(cursor + (hash(i, seed, 1) - 0.5) * 3);
    cursor += widths[i] + gap;
    let depth = i % 2 ? 0.15 + hash(i, seed, 2) * 0.2 : 0.75 + hash(i, seed, 3) * 0.25;
    let feet = Math.round(yb + 7 + depth * (yf - yb - 20));
    return { kind, x, feet, flip: kind !== "graze" && hash(i, seed, 4) > 0.75, seed: i * 7 + seed, back: depth < 0.5 };
  });
  let hopper = rams.find((r) => r.kind === "stand" && !r.back) ?? rams[0];
  hopper.hops = true;
  hopper.kind = "stand";
  return rams.sort((a, b) => a.feet - b.feet);
}

function sprite(b: Buf, rows: string[], x: number, feet: number, flip: boolean, dim: number, ear: boolean) {
  let w = rows[0].length;
  let top = feet - rows.length;
  rows.forEach((row, j) =>
    [...row].forEach((ch, i) => {
      if (ch === ".") return;
      if (ch === "W" && hash(i, j, 3) > 0.78) ch = "w";
      if (ch === "E" && ear) ch = "F";
      let col = RAM[ch];
      if (dim) col = scale(col, dim);
      b.set(flip ? x + w - 1 - i : x + i, top + j, col);
    }),
  );
}

function herdFrame(b: Buf, rams: Ram[], front: Buf, t: number) {
  for (let r of rams) {
    let pose = r.kind;
    let dy = 0;
    let ear = false;
    if (r.kind === "graze") {
      let per = 7 + hash(r.seed, 1) * 4;
      if ((t + hash(r.seed, 2) * per) % per < 1.6) pose = "stand";
    }
    if (r.kind === "stand" && !r.hops) {
      let per = 4 + hash(r.seed, 3) * 3;
      ear = (t + hash(r.seed, 4) * per) % per < 0.25;
    }
    let dust = -1;
    if (r.hops) {
      let per = 9;
      let ph = (t + per * 10 - 2.2) % per;
      if (ph < 0.15) dy = 1;
      else if (ph < 0.65) {
        pose = "hop";
        dy = -Math.round(Math.sin(((ph - 0.15) / 0.5) * Math.PI) * 6);
      } else if (ph < 0.75) dy = 1;
      if (ph >= 0.65 && ph < 1.3) dust = (ph - 0.65) / 0.65;
      ear = ph > 3 && ph < 3.25;
    }
    sprite(b, RAMS[pose], r.x, r.feet + dy, r.flip, r.back ? 0.86 : 0, ear);
    if (dust >= 0) {
      let w = RAMS.stand[0].length;
      for (let k = 0; k < 10; k++) {
        let side = k % 2 ? 1 : -1;
        let spread = 2 + dust * (6 + hash(k, r.seed, 9) * 6);
        let x = Math.round((r.flip ? r.x + w - 10 : r.x + 9) + side * spread);
        let y = Math.round(r.feet - 1 - (k % 3 === 0 ? 1 : 0) - dust * (k % 4 === 0 ? 2 : 0));
        if (bayer(x, y) < dust * 1.1) continue;
        b.set(x, y, DUST[dust < 0.4 ? 0 : 1]);
      }
    }
  }
  let f = front.d;
  let d = b.d;
  for (let i = 0; i < f.length; i += 4)
    if (f[i + 3] === 255) {
      d[i] = f[i];
      d[i + 1] = f[i + 1];
      d[i + 2] = f[i + 2];
    }
}

function corralScene(b: Buf, w: number, phone: boolean, panel: Rect | null, avoid: Rect[]): State {
  let p = panel ?? { x0: 0, x1: w, y0: b.h - 20, y1: b.h };
  let horizon = p.y0 - (phone ? 46 : 58);
  sky(b, w, 0, horizon + 2, INK.sky);
  let list = stars(b, w, 4, horizon - 10, phone ? 0.13 : 0.11, avoid, 21);
  mesa(b, w, Math.round(w * (phone ? 0.5 : 0.66)), w + 14, horizon - (phone ? 14 : 20), horizon + 2, 4);
  mesa(b, w, Math.round(w * (phone ? 0.02 : 0.42)), Math.round(w * (phone ? 0.26 : 0.58)), horizon - 9, horizon + 2, 3);
  hills(b, w, b.h, [
    { base: horizon + 5, amp: 3 },
    { base: horizon + 12, amp: 2 },
  ]);
  let yb = horizon + (phone ? 12 : 16);
  let yf = p.y0 - 4;
  let x0 = Math.round(w * (phone ? 0.36 : 0.6));
  let x1 = Math.round(w * (phone ? 0.86 : 0.94));
  let fx = Math.round(w * (phone ? 0.16 : 0.52));
  let fy = yf - 2;
  pool(b, fx, fy, 40);
  let front = corral(b, w, b.h, { x0, x1, yb, yf, gap: phone ? 22 : 28, small: phone });
  logs(b, fx, fy, phone ? 7 : 8);
  return {
    base: b,
    stars: list,
    front,
    rams: herd(x0, x1, yb, yf, phone ? 3 : 4, 5),
    fire: { x: fx, y: fy, small: phone },
  };
}

function horizonScene(b: Buf, w: number): State {
  let h = b.h;
  let horizon = Math.round(h * 0.62);
  sky(b, w, 0, horizon + 2, INK.sky);
  let list = stars(b, w, 2, horizon - 6, 0.12, [], 33);
  for (let s of list) if (s.tw) glyph(b, s.x, s.y, s.col, s.kind);
  mesa(b, w, Math.round(w * 0.06), Math.round(w * 0.3), horizon - 10, horizon + 2, 6);
  mesa(b, w, Math.round(w * 0.7), w + 10, horizon - 14, horizon + 2, 7);
  hills(b, w, h, [
    { base: horizon + 3, amp: 3 },
    { base: horizon + 9, amp: 2 },
  ]);
  dissolve(b, w, 0, 6, true);
  dissolve(b, w, h - 8, h);
  return { base: b, stars: [] };
}

function nightScene(b: Buf, w: number, avoid: Rect[]): State {
  let h = b.h;
  sky(b, w, 0, h, INK.sky.slice(0, 4).reverse());
  let list = stars(b, w, 2, h - 10, 0.1, avoid, 21);
  dissolve(b, w, h - 14, h);
  return { base: b, stars: list };
}

function within(canvas: HTMLCanvasElement, el: Element, scale: number): Rect {
  let a = canvas.parentElement!.getBoundingClientRect();
  let r = el.getBoundingClientRect();
  return {
    x0: Math.floor((r.left - a.left) / scale),
    y0: Math.floor((r.top - a.top) / scale),
    x1: Math.ceil((r.right - a.left) / scale),
    y1: Math.ceil((r.bottom - a.top) / scale),
  };
}

/**
 * Paint every `canvas[data-world]`: the corral, the night sky or the horizon.
 * @example worlds()
 */
export function worlds(): void {
  for (let canvas of document.querySelectorAll<HTMLCanvasElement>("canvas[data-world]")) {
    let name = canvas.dataset.world;
    mount<State>(canvas, {
      scale: (width) => (width < 640 ? 2 : 3),
      setup(w, h, width) {
        let phone = width < 640;
        let scale = phone ? 2 : 3;
        let b = new Buf(w, h);
        let avoid = canvas.dataset.avoid
          ? [...document.querySelectorAll(canvas.dataset.avoid)].flatMap((el) => [...el.children].map((kid) => within(canvas, kid, scale)))
          : [];
        let panel = canvas.dataset.panel ? document.querySelector(canvas.dataset.panel) : null;
        if (name === "corral") return corralScene(b, w, phone, panel && within(canvas, panel, scale), avoid);
        if (name === "night") return nightScene(b, w, avoid);
        return horizonScene(b, w);
      },
      frame(state, t) {
        let b = state.base.copy();
        if (state.rams && state.front) herdFrame(b, state.rams, state.front, t);
        twinkle(b, state.stars, t);
        if (state.fire) fire(b, state.fire, t);
        return b;
      },
      animate: name !== "horizon",
      fps: 10,
      rest: 0.4,
    });
  }
}
