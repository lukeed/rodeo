/**
 * Mark the "on this page" link of the section in view with `aria-current`.
 * @example spy()
 */
export function spy(): void {
  let links = [...document.querySelectorAll<HTMLAnchorElement>(".toc a[href^='#']")];
  let heads = links.map((link) => document.getElementById(decodeURIComponent(link.hash.slice(1))));
  if (!links.length || heads.some((head) => !head)) return;

  let current = -1;
  let update = () => {
    let line = Math.min(160, innerHeight * 0.3);
    let end = scrollY + innerHeight >= document.documentElement.scrollHeight - 2;
    let index = end ? heads.length - 1 : heads.findLastIndex((head) => head!.getBoundingClientRect().top <= line);
    index = Math.max(0, index);
    if (index === current) return;
    links[current]?.removeAttribute("aria-current");
    links[index].setAttribute("aria-current", "true");
    current = index;
  };

  let queued = false;
  let schedule = () => {
    if (queued) return;
    queued = true;
    requestAnimationFrame(() => {
      queued = false;
      update();
    });
  };
  addEventListener("scroll", schedule, { passive: true });
  addEventListener("resize", schedule);
  update();
}
