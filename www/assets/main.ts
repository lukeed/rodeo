import * as toc from "./js/toc";
import * as tui from "./js/tui";
import * as world from "./js/world";

async function copy(button: HTMLElement, text: string) {
  let label = button.textContent;
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = "copied";
  } catch {
    button.textContent = "copy failed";
  }
  setTimeout(() => (button.textContent = label), 1600);
}

/** Safari allows a clipboard write only during the click, so the fetch goes to ClipboardItem as a promise. */
async function copyPage(button: HTMLElement, url: string) {
  let label = button.textContent;
  let text = fetch(url)
    .then((res) => {
      if (!res.ok) throw new Error(`${url}: ${res.status}`);
      return res.text();
    })
    .then((body) => new Blob([body], { type: "text/plain" }));
  try {
    await navigator.clipboard.write([new ClipboardItem({ "text/plain": text })]);
    button.textContent = "copied";
  } catch {
    button.textContent = "copy failed";
  }
  setTimeout(() => (button.textContent = label), 1600);
}

document.addEventListener("click", (event) => {
  let target = event.target as HTMLElement;
  let page = target.closest<HTMLElement>("[data-copy-page]");
  if (page) return void copyPage(page, page.dataset.copyPage!);
  let button = target.closest<HTMLElement>("[data-copy]");
  if (button) void copy(button, button.dataset.copy!);
});

let band = document.querySelector<HTMLElement>(".hband");
let panel = document.querySelector<HTMLElement>(".tui");
if (band && panel) {
  let fit = () => (band.style.height = Math.round(panel.getBoundingClientRect().top + scrollY + 60) + "px");
  fit();
  new ResizeObserver(fit).observe(document.body);
}

toc.spy();
tui.screens();
world.worlds();
