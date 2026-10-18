// Browser-side filtering of a rendered list: a text box over data-text of each row and
// status chips over data-status; the count follows. The server renders the full list.
function apply(root: HTMLElement): void {
  const q = (root.querySelector<HTMLInputElement>("input[data-filter]")?.value ?? "").trim().toLowerCase();
  const active = new Set([...root.querySelectorAll<HTMLElement>("[data-chip][aria-pressed='true']")].map((c) => c.dataset.chip!));
  let shown = 0;
  for (const row of root.querySelectorAll<HTMLElement>("[data-row-text]")) {
    const text = row.dataset.rowText ?? "";
    const status = row.dataset.status ?? "";
    const ok = (q === "" || text.includes(q)) && (active.size === 0 || active.has(status));
    row.hidden = !ok;
    if (ok) shown++;
  }
  // The count may sit in the page header, outside the filter root (then there is one root per page).
  const count = root.querySelector<HTMLElement>("[data-filter-count]") ?? document.querySelector<HTMLElement>("[data-filter-count]");
  if (count) count.textContent = String(shown);
  const empty = root.querySelector<HTMLElement>("[data-filter-empty]");
  if (empty) empty.hidden = shown > 0;
}

export function initFilters(root: ParentNode = document): void {
  for (const box of root.querySelectorAll<HTMLElement>("[data-filter-root]")) {
    if (box.dataset.filterReady) continue;
    box.dataset.filterReady = "1";
    box.querySelector<HTMLElement>("[data-filter-ui]")?.removeAttribute("hidden");
    box.addEventListener("input", () => apply(box));
    box.addEventListener("click", (e) => {
      const chip = (e.target as HTMLElement).closest<HTMLElement>("[data-chip]");
      if (!chip) return;
      e.preventDefault();
      chip.setAttribute("aria-pressed", chip.getAttribute("aria-pressed") === "true" ? "false" : "true");
      apply(box);
    });
    apply(box);
  }
}
document.addEventListener("astro:page-load", () => initFilters());
initFilters();
