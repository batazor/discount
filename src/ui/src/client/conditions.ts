// Builder conditions: show a condition's inputs while it is on, and keep the "who gets it"
// sentence current. Works for any form carrying [data-conditions].
import { fieldsOf } from "../model/fields";
import { describeConditions } from "../model/summary";

const localDate = (ts: number) => new Date(ts * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

export function initConditions(root: ParentNode = document): void {
  for (const form of root.querySelectorAll<HTMLFormElement>("form[data-conditions]")) {
    if (form.dataset.conditionsReady) continue;
    form.dataset.conditionsReady = "1";
    const refresh = () => {
      for (const box of form.querySelectorAll<HTMLElement>("[data-cond]")) {
        const input = form.querySelector<HTMLInputElement>(`input[name="b.${box.dataset.cond}"]`);
        box.hidden = !(input?.checked ?? false);
      }
      const text = describeConditions(fieldsOf(new FormData(form)), localDate);
      for (const el of form.querySelectorAll<HTMLElement>("[data-conditions-text]")) el.textContent = text || "every cart";
    };
    form.addEventListener("change", refresh);
    form.addEventListener("input", refresh);
    refresh();
  }
}
document.addEventListener("astro:page-load", () => initConditions());
initConditions();
