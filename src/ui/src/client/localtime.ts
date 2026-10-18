// Local time at the edge: <time data-ts> is rewritten in the viewer's zone (UTC stays in
// the tooltip) and datetime-local inputs that carry a UTC value are shifted to local, with
// the zone offset posted back in tz_offset_minutes so the server reads them correctly.
const offset = new Date().getTimezoneOffset(); // minutes to add to local to get UTC
const zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
const fmt = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

function pad(n: number): string { return String(n).padStart(2, "0"); }
function localInputValue(utcSeconds: number): string {
  const d = new Date(utcSeconds * 1000);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

export function localize(root: ParentNode = document): void {
  for (const t of root.querySelectorAll<HTMLElement>("time[data-ts]:not([data-local])")) {
    t.dataset.local = "1";
    t.textContent = fmt.format(new Date(Number(t.dataset.ts) * 1000));
  }
  for (const input of root.querySelectorAll<HTMLInputElement>("input[type='datetime-local'][data-utc]")) {
    input.removeAttribute("data-utc");
    if (input.value) {
      const ms = Date.parse(`${input.value}Z`);
      if (!Number.isNaN(ms)) input.value = localInputValue(ms / 1000);
    }
  }
  for (const form of root.querySelectorAll<HTMLFormElement>("form")) {
    if (!form.querySelector("input[type='datetime-local']") || form.querySelector("input[name='tz_offset_minutes']")) continue;
    const h = document.createElement("input");
    h.type = "hidden"; h.name = "tz_offset_minutes"; h.value = String(offset);
    form.appendChild(h);
  }
  for (const z of root.querySelectorAll<HTMLElement>("[data-zone]")) z.textContent = zone;
}
document.addEventListener("astro:page-load", () => localize());
localize();
