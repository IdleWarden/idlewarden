import { mkdir, readFile, writeFile, copyFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const out = process.argv[2] ?? join(here, "out");
const mod = join(out, "idlewarden-bridge");

await mkdir(mod, { recursive: true });
const parts = await Promise.all(
  [
    join(here, "idlewarden-bridge", "bridge.js"),
    join(here, "cookie-clicker", "mod.js"),
  ].map((path) => readFile(path, "utf8")),
);
await writeFile(join(mod, "main.js"), parts.join("\n"), "utf8");
await copyFile(join(here, "cookie-clicker", "info.txt"), join(mod, "info.txt"));
console.log(mod);
