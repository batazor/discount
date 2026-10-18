// Progressive enhancement for repeated rows (rules, cart lines, attributes, uses) and
// variant pickers. Without JavaScript the server-rendered rows still submit; with it,
// rows can be added and removed and only the chosen variant's fields are shown.
// Enter/exit use `motion` (design.md, decision 10) unless the user prefers reduced motion;
// the row is in the DOM before any animation starts.
import { animate } from "motion";

const reduced = () => window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function variantValue(row: Element): string | null {
  const picker = row.querySelector<HTMLElement>("[data-variant-select]");
  if (!picker) return null;
  if (picker instanceof HTMLSelectElement) return picker.value;
  return picker.querySelector<HTMLInputElement>("input[type='radio']:checked")?.value ?? "";
}

function showVariant(row: Element): void {
  const value = variantValue(row);
  if (value === null) return;
  for (const el of row.querySelectorAll<HTMLElement>("[data-variant]")) el.hidden = el.dataset.variant !== value;
}

function nextIndex(list: Element): number {
  let max = -1;
  for (const row of list.querySelectorAll<HTMLElement>("[data-row]")) max = Math.max(max, Number(row.dataset.row));
  return max + 1;
}

function addRow(list: HTMLElement): void {
  const tpl = list.querySelector<HTMLTemplateElement>("template[data-row-template]");
  if (!tpl) return;
  const i = nextIndex(list);
  const html = tpl.innerHTML.replaceAll("__i__", String(i));
  const holder = document.createElement("div");
  holder.innerHTML = html;
  const row = holder.firstElementChild as HTMLElement | null;
  if (!row) return;
  row.dataset.row = String(i);
  list.querySelector("[data-rows]")?.appendChild(row);
  list.querySelector("[data-empty-rules]")?.remove();
  showVariant(row);
  for (const btn of row.querySelectorAll<HTMLElement>("[data-remove-row]")) btn.hidden = false;
  if (!reduced()) animate(row, { opacity: [0, 1], y: [-6, 0] }, { duration: 0.18 });
}

async function removeRow(row: HTMLElement): Promise<void> {
  if (!reduced()) await animate(row, { opacity: 0, height: 0 }, { duration: 0.15 }).finished;
  row.remove();
}

export function initRows(root: ParentNode = document): void {
  for (const list of root.querySelectorAll<HTMLElement>("[data-row-list]")) {
    if (list.dataset.rowsReady) continue;
    list.dataset.rowsReady = "1";
    list.querySelectorAll("[data-row]").forEach(showVariant);
    list.addEventListener("change", (e) => {
      const t = e.target as HTMLElement;
      if (t.matches("[data-variant-select], [data-variant-select] input")) showVariant(t.closest("[data-row]")!);
    });
    list.addEventListener("click", (e) => {
      const t = (e.target as HTMLElement).closest<HTMLElement>("[data-add-row],[data-remove-row]");
      if (!t) return;
      e.preventDefault();
      if (t.hasAttribute("data-add-row")) addRow(list);
      else void removeRow(t.closest<HTMLElement>("[data-row]")!);
    });
    for (const btn of list.querySelectorAll<HTMLElement>("[data-add-row],[data-remove-row]")) btn.hidden = false;
  }
}

document.addEventListener("astro:page-load", () => initRows());
