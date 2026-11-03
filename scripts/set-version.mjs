// Sets the app version everywhere: `npm run version:set 0.2.0`
import { readFileSync, writeFileSync } from "node:fs";

const v = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[\w.]+)?$/.test(v ?? "")) {
  console.error("usage: npm run version:set <major.minor.patch>");
  process.exit(1);
}

const cargo = "Cargo.toml";
writeFileSync(cargo, readFileSync(cargo, "utf8").replace(/(\[workspace\.package\][^[]*?\nversion = )"[^"]+"/, `$1"${v}"`));

const pkg = JSON.parse(readFileSync("package.json", "utf8"));
pkg.version = v;
writeFileSync("package.json", JSON.stringify(pkg, null, 2) + "\n");

console.log(`Version set to ${v}. Next: cargo update -w, commit, then git tag v${v} && git push --tags`);
