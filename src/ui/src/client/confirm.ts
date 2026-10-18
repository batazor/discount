// Forms with data-confirm open the page's <dialog id="confirm-dialog"> instead of the
// browser's confirm(); the dialog's confirm button submits the form.
let pending: HTMLFormElement | null = null;

// With scripts the dialog replaces the browser's confirm(): the inline fallback is removed
// so an operator is asked once, not twice.
export function initConfirm(root: ParentNode = document): void {
  for (const f of root.querySelectorAll<HTMLFormElement>("form[data-confirm][onsubmit]")) f.removeAttribute("onsubmit");
}
document.addEventListener("astro:page-load", () => initConfirm());
initConfirm();

function dialog(): HTMLDialogElement | null {
  return document.getElementById("confirm-dialog") as HTMLDialogElement | null;
}

// Capture phase: Astro's ClientRouter also listens for submit (bubbling) and would send the
// form itself; preventing the default here first makes it stand down.
document.addEventListener("submit", (e) => {
  const form = e.target as HTMLFormElement;
  if (!form.hasAttribute("data-confirm") || form.dataset.confirmed === "1") return;
  const d = dialog();
  if (!d || typeof d.showModal !== "function") return; // fall back to onsubmit confirm()
  e.preventDefault();
  pending = form;
  d.querySelector<HTMLElement>("[data-confirm-title]")!.textContent = form.dataset.confirmTitle ?? "Are you sure?";
  d.querySelector<HTMLElement>("[data-confirm-text]")!.textContent = form.dataset.confirm ?? "";
  d.querySelector<HTMLElement>("[data-confirm-ok]")!.textContent = form.dataset.confirmLabel ?? "Confirm";
  d.returnValue = "";
  d.showModal();
}, true);

document.addEventListener("close", (e) => {
  const d = e.target as HTMLDialogElement;
  if (d.id !== "confirm-dialog" || !pending) return;
  const form = pending;
  pending = null;
  if (d.returnValue !== "ok") return;
  form.dataset.confirmed = "1";
  form.removeAttribute("onsubmit");
  form.requestSubmit();
}, true);
