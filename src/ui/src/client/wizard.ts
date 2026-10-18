// The campaign wizard: one form, shown one step at a time, with a live summary. Without
// this script every step is visible and the form still submits.
import { fieldsOf } from "../model/fields";
import { summarize } from "../model/summary";

const localDate = (ts: number) => new Date(ts * 1000).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" });

export function initWizard(root: ParentNode = document): void {
  const form = root.querySelector<HTMLFormElement>("form[data-wizard]");
  if (!form || form.dataset.wizardReady) return;
  form.dataset.wizardReady = "1";
  const steps = [...form.querySelectorAll<HTMLElement>("[data-step]")];
  const bar = [...document.querySelectorAll<HTMLElement>("[data-step-link]")];
  const summaries = [...document.querySelectorAll<HTMLElement>("[data-summary-text], [data-summary-text-review]")];
  let current = Math.max(0, steps.findIndex((s) => s.dataset.step === new URL(location.href).hash.slice(1)));

  const show = (i: number) => {
    current = Math.min(Math.max(i, 0), steps.length - 1);
    steps.forEach((s, k) => { s.hidden = k !== current; });
    bar.forEach((b, k) => { b.setAttribute("aria-current", k === current ? "step" : "false"); b.classList.toggle("done", k < current); });
    for (const n of form.querySelectorAll<HTMLElement>("[data-next]")) n.hidden = current === steps.length - 1;
    for (const p of form.querySelectorAll<HTMLElement>("[data-back]")) p.hidden = current === 0;
    for (const s of form.querySelectorAll<HTMLElement>("[data-submit]")) s.hidden = current !== steps.length - 1;
    history.replaceState(null, "", `#${steps[current]!.dataset.step}`);
    steps[current]!.querySelector<HTMLElement>("input:not([type=hidden]), select, textarea")?.focus({ preventScroll: true });
    window.scrollTo({ top: 0 });
  };
  const refresh = () => {
    const f = fieldsOf(new FormData(form));
    const text = summarize(f, localDate);
    for (const el of summaries) el.textContent = text;
    for (const box of form.querySelectorAll<HTMLElement>("[data-cond]")) {
      const input = form.querySelector<HTMLInputElement>(`input[name="b.${box.dataset.cond}"]`);
      box.hidden = !(input?.checked ?? false);
    }
    const needsCode = form.querySelector<HTMLInputElement>('input[name="b.needs_code"]')?.checked ?? false;
    for (const el of form.querySelectorAll<HTMLElement>("[data-needs-code-hint]")) el.hidden = needsCode;
  };

  form.addEventListener("click", (e) => {
    const t = (e.target as HTMLElement).closest<HTMLElement>("[data-next],[data-back]");
    if (!t) return;
    e.preventDefault();
    show(current + (t.hasAttribute("data-next") ? 1 : -1));
  });
  bar.forEach((b, k) => b.addEventListener("click", (e) => { e.preventDefault(); show(k); }));
  form.addEventListener("input", refresh);
  form.addEventListener("change", refresh);
  // Enter inside a step moves on instead of submitting early.
  form.addEventListener("keydown", (e) => {
    if (e.key === "Enter" && (e.target as HTMLElement).tagName === "INPUT" && current < steps.length - 1) { e.preventDefault(); show(current + 1); }
  });
  for (const el of form.querySelectorAll<HTMLElement>("[data-wizard-only]")) el.hidden = false;
  refresh();
  show(current);
}
document.addEventListener("astro:page-load", () => initWizard());
initWizard();
