// @ts-check
import { defineConfig } from "astro/config";
import node from "@astrojs/node";
import tailwindcss from "@tailwindcss/vite";

// Server-rendered on every request (design.md, decision 1): pages fetch from the discount
// service server-to-server, so no CORS and no API URL in the browser.
export default defineConfig({
  output: "server",
  adapter: node({ mode: "standalone" }),
  server: { port: 4321, host: true },
  vite: { plugins: [tailwindcss()] },
});
