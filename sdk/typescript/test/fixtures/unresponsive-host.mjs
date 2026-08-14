import { spawn } from "node:child_process";

const grandchild = spawn(
  process.execPath,
  ["-e", "setInterval(() => {}, 1000)"],
  { stdio: "ignore", windowsHide: true },
);

process.stdout.write(`${grandchild.pid}\n`);
if (process.argv[2] === "exit-parent") {
  grandchild.unref();
  process.exit(0);
}
process.stdin.resume();
setInterval(() => {}, 1000);
