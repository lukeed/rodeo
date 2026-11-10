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

document.addEventListener("click", (event) => {
  let button = (event.target as HTMLElement).closest<HTMLElement>("[data-copy]");
  if (button) void copy(button, button.dataset.copy!);
});

let band = document.querySelector<HTMLElement>(".hband");
let panel = document.querySelector<HTMLElement>(".tui");
if (band && panel) {
  let fit = () => (band.style.height = Math.round(panel.getBoundingClientRect().top + scrollY + 60) + "px");
  fit();
  new ResizeObserver(fit).observe(document.body);
}

tui.screens();
world.worlds();
