export type RGB = [number, number, number];

let imul = Math.imul;

export function hash(a: number, b = 0, c = 0): number {
  let h = imul(a | 0, 0x27d4eb2d) ^ imul(b | 0, 0x165667b1) ^ imul(c | 0, 0x9e3779b1);
  h = imul(h ^ (h >>> 15), 0x85ebca6b);
  h = imul(h ^ (h >>> 13), 0xc2b2ae35);
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

let smooth = (t: number) => t * t * (3 - 2 * t);

export function noise1(x: number, seed = 0): number {
  let i = Math.floor(x);
  let f = smooth(x - i);
  let a = hash(i, seed, 7);
  let b = hash(i + 1, seed, 7);
  return a + (b - a) * f;
}

export function noise2(x: number, y: number, seed = 0): number {
  let ix = Math.floor(x);
  let iy = Math.floor(y);
  let fx = smooth(x - ix);
  let fy = smooth(y - iy);
  let a = hash(ix, iy, seed);
  let b = hash(ix + 1, iy, seed);
  let c = hash(ix, iy + 1, seed);
  let d = hash(ix + 1, iy + 1, seed);
  return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
}

export function streak(x: number, y: number, t: number, min = 3, spread = 7): boolean {
  let len = min + Math.floor(hash(y, 911) * spread);
  return hash(Math.floor((x + hash(y, 17) * 64) / len), y * 7 + 3) < t;
}

const BAYER = [0, 8, 2, 10, 12, 4, 14, 6, 3, 11, 1, 9, 15, 7, 13, 5];

export function bayer(x: number, y: number): number {
  return (BAYER[(y & 3) * 4 + (x & 3)] + 0.5) / 16;
}

export function rgb(hex: string): RGB {
  let n = parseInt(hex.slice(1), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** The `?t=` query pins every animation to that second, for screenshots. */
export const pinned: number | null = (() => {
  let value = new URLSearchParams(location.search).get("t");
  return value == null ? null : Number(value);
})();

let motion = matchMedia("(prefers-reduced-motion: reduce)");

export function still(): boolean {
  return pinned != null || motion.matches;
}

export function onMotion(fn: () => void): void {
  motion.addEventListener("change", fn);
}

export class Buf {
  d: Uint8ClampedArray;
  constructor(
    public w: number,
    public h: number,
  ) {
    this.d = new Uint8ClampedArray(w * h * 4);
  }

  set(x: number, y: number, c: RGB): void {
    x |= 0;
    y |= 0;
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    let i = (y * this.w + x) * 4;
    this.d[i] = c[0];
    this.d[i + 1] = c[1];
    this.d[i + 2] = c[2];
    this.d[i + 3] = 255;
  }

  add(x: number, y: number, c: RGB, k: number): void {
    x |= 0;
    y |= 0;
    if (x < 0 || y < 0 || x >= this.w || y >= this.h) return;
    let i = (y * this.w + x) * 4;
    this.d[i] += c[0] * k;
    this.d[i + 1] += c[1] * k;
    this.d[i + 2] += c[2] * k;
  }

  copy(): Buf {
    let b = new Buf(this.w, this.h);
    b.d.set(this.d);
    return b;
  }
}

export type Scene<S> = {
  scale(width: number): number;
  setup(w: number, h: number, width: number): S;
  frame(state: S, t: number): Buf;
  animate: boolean;
  fps: number;
  rest: number;
};

export function mount<S>(canvas: HTMLCanvasElement, scene: Scene<S>): void {
  let ctx = canvas.getContext("2d")!;
  let box = canvas.parentElement!;
  let state: S | null = null;
  let img: ImageData | null = null;
  let visible = true;
  let raf = 0;
  let last = -1;
  let t0 = performance.now();

  let paint = (t: number) => {
    if (!state || !img) return;
    img.data.set(scene.frame(state, t).d);
    ctx.putImageData(img, 0, 0);
  };

  let resize = () => {
    let rect = box.getBoundingClientRect();
    let scale = scene.scale(rect.width);
    let w = Math.ceil(rect.width / scale);
    let h = Math.ceil(rect.height / scale);
    if (w < 1 || h < 1) return;
    canvas.width = w;
    canvas.height = h;
    canvas.style.width = w * scale + "px";
    canvas.style.height = h * scale + "px";
    img = ctx.createImageData(w, h);
    state = scene.setup(w, h, rect.width);
    paint(still() ? (pinned ?? scene.rest) : (performance.now() - t0) / 1000);
  };

  let loop = (now: number) => {
    raf = 0;
    if (!visible || still() || !scene.animate) return;
    let step = Math.floor(((now - t0) / 1000) * scene.fps);
    if (step !== last) {
      last = step;
      paint(step / scene.fps);
    }
    raf = requestAnimationFrame(loop);
  };

  let start = () => {
    if (!raf && visible && !still() && scene.animate) raf = requestAnimationFrame(loop);
  };

  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    start();
  }).observe(canvas);

  let timer = 0;
  new ResizeObserver(() => {
    clearTimeout(timer);
    timer = window.setTimeout(resize, state ? 120 : 0);
  }).observe(box);

  onMotion(() => (still() ? paint(scene.rest) : start()));
  resize();
  start();
}
