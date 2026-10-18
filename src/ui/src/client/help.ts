// Shows the description of the option currently chosen in a select (markup rendered by
// Field.astro); without scripts, every description is listed.
function sync(select: HTMLSelectElement): void {
  const box = select.closest("label")?.querySelector<HTMLElement>("[data-option-help]");
  if (!box) return;
  for (const el of box.querySelectorAll<HTMLElement>("[data-for]")) el.hidden = el.dataset.for !== select.value;
}
export function initOptionHelp(root: ParentNode = document): void {
  for (const s of root.querySelectorAll<HTMLSelectElement>("select[data-has-help]")) sync(s);
}
document.addEventListener("change", (e) => {
  const t = e.target as HTMLElement;
  if (t instanceof HTMLSelectElement && t.hasAttribute("data-has-help")) sync(t);
});
document.addEventListener("astro:page-load", () => initOptionHelp());
initOptionHelp();
