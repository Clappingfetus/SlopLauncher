import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import path from "node:path";
const root = process.cwd();
const env = { ...process.env };
const originalPath =
  Object.entries(env).find(([key]) => key.toLowerCase() === "path")?.[1] ?? "";
for (const key of Object.keys(env)) {
  if (key.toLowerCase() === "path") delete env[key];
}
env.Path = path.dirname(process.execPath) + path.delimiter + originalPath;
const local = path.join(root, ".tools", "cargo");
if (existsSync(path.join(local, "bin", "cargo.exe"))) {
  env.CARGO_HOME = local;
  env.RUSTUP_HOME = path.join(root, ".tools", "rustup");
  env.Path = path.join(local, "bin") + path.delimiter + env.Path;
}
const cli = path.join(root, "node_modules", "@tauri-apps", "cli", "tauri.js");
const child = spawn(process.execPath, [cli, ...process.argv.slice(2)], {
  stdio: "inherit",
  env,
});
child.on("exit", (code) => process.exit(code ?? 1));
