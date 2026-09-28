import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join, relative, resolve, sep } from "node:path";

export type Domain = "core" | "platform" | "presentation" | "ui" | "tui";

export interface Violation {
  file: string;
  line: number;
  from: Domain;
  to: Domain;
}

const FORBIDDEN: Record<Domain, ReadonlySet<Domain>> = {
  core: new Set(["platform", "presentation", "ui", "tui"]),
  platform: new Set(["presentation", "ui", "tui"]),
  presentation: new Set(["platform", "ui", "tui"]),
  ui: new Set(["platform", "tui"]),
  tui: new Set(["core", "platform", "ui"]),
};

const DOMAINS = new Set<Domain>(["core", "platform", "presentation", "ui", "tui"]);

function isDomain(value: string): value is Domain {
  return DOMAINS.has(value as Domain);
}

function modulePath(file: string): string[] | null {
  const normalized = file.split(sep).join("/");
  const marker = "/src/";
  const srcIndex = normalized.lastIndexOf(marker);
  const underSrc = srcIndex >= 0 ? normalized.slice(srcIndex + marker.length) : normalized.replace(/^src\//, "");
  const parts = underSrc.split("/");
  const filename = parts.pop();
  if (!filename?.endsWith(".rs")) return null;

  const stem = filename.slice(0, -3);
  if (stem !== "mod" && stem !== "lib" && stem !== "main") parts.push(stem);
  return parts;
}

function sourceDomain(file: string): Domain | null {
  const normalized = file.split(sep).join("/");
  for (const [crateName, domain] of [
    ["cclover-core", "core"],
    ["cclover-presentation", "presentation"],
    ["cclover-tui", "tui"],
    ["cclover-desktop-ui", "ui"],
    ["cclover-desktop", "ui"],
  ] as const) {
    if (normalized.includes(`/crates/${crateName}/src/`) || normalized.startsWith(`crates/${crateName}/src/`)) {
      return domain;
    }
  }
  const path = modulePath(file);
  const root = path?.[0];
  return root && isDomain(root) ? root : null;
}

/** Replace comments and string contents with spaces while preserving newlines and offsets. */
export function stripRustNonCode(source: string): string {
  const out = [...source];
  let i = 0;

  const blank = (start: number, end: number) => {
    for (let p = start; p < end; p++) if (out[p] !== "\n") out[p] = " ";
  };

  while (i < source.length) {
    if (source.startsWith("//", i)) {
      const end = source.indexOf("\n", i + 2);
      const stop = end < 0 ? source.length : end;
      blank(i, stop);
      i = stop;
      continue;
    }

    if (source.startsWith("/*", i)) {
      const start = i;
      let depth = 1;
      i += 2;
      while (i < source.length && depth > 0) {
        if (source.startsWith("/*", i)) {
          depth++;
          i += 2;
        } else if (source.startsWith("*/", i)) {
          depth--;
          i += 2;
        } else {
          i++;
        }
      }
      blank(start, i);
      continue;
    }

    const raw = source.slice(i).match(/^(?:br|rb|r)(#{0,255})"/);
    if (raw) {
      const start = i;
      const hashes = raw[1] ?? "";
      i += raw[0].length;
      const close = `"${hashes}`;
      const end = source.indexOf(close, i);
      i = end < 0 ? source.length : end + close.length;
      blank(start, i);
      continue;
    }

    const stringPrefix = source.startsWith('b"', i) ? 1 : source[i] === '"' ? 0 : -1;
    if (stringPrefix >= 0) {
      const start = i;
      i += stringPrefix + 1;
      while (i < source.length) {
        if (source[i] === "\\") i += 2;
        else if (source[i] === '"') {
          i++;
          break;
        } else i++;
      }
      blank(start, i);
      continue;
    }

    i++;
  }

  return out.join("");
}

function lineAt(source: string, offset: number): number {
  let line = 1;
  for (let i = 0; i < offset; i++) if (source.charCodeAt(i) === 10) line++;
  return line;
}

function topLevelUseRoots(body: string): string[] {
  const roots: string[] = [];
  let depth = 0;
  let start = 0;

  const pushBranch = (branch: string) => {
    const match = branch.trim().match(/^([A-Za-z_][A-Za-z0-9_]*)\b/);
    if (match) roots.push(match[1]);
  };

  for (let i = 0; i <= body.length; i++) {
    const ch = body[i];
    if (ch === "{") depth++;
    else if (ch === "}") depth--;
    else if ((ch === "," && depth === 0) || i === body.length) {
      pushBranch(body.slice(start, i));
      start = i + 1;
    }
  }
  return roots;
}

function bracedUseDependencies(code: string, module: string[]): Array<{ to: Domain; offset: number }> {
  const found: Array<{ to: Domain; offset: number }> = [];
  const usePattern = /\buse\s+((?:crate|super)(?:\s*::\s*super)*)\s*::\s*\{/g;
  let match: RegExpExecArray | null;

  while ((match = usePattern.exec(code))) {
    const prefix = match[1].replace(/\s/g, "");
    let base: string[];
    if (prefix === "crate") {
      base = [];
    } else {
      const supers = prefix.split("::").length;
      base = module.slice(0, Math.max(0, module.length - supers));
      if (supers > module.length) continue;
    }

    let depth = 1;
    const open = match.index + match[0].lastIndexOf("{");
    let close = open + 1;
    while (close < code.length && depth > 0) {
      if (code[close] === "{") depth++;
      else if (code[close] === "}") depth--;
      close++;
    }
    if (depth !== 0) continue;

    if (base.length === 0) {
      for (const root of topLevelUseRoots(code.slice(open + 1, close - 1))) {
        if (isDomain(root)) found.push({ to: root, offset: match.index });
      }
    }
    usePattern.lastIndex = close;
  }

  return found;
}

export function findViolationsInSource(source: string, file: string): Violation[] {
  const from = sourceDomain(file);
  const module = modulePath(file);
  if (!from || !module) return [];

  const code = stripRustNonCode(source);
  const candidates: Array<{ to: Domain; offset: number }> = [];

  const cratePath = /\bcrate\s*::\s*(core|platform|presentation|ui)\b/g;
  let match: RegExpExecArray | null;
  while ((match = cratePath.exec(code))) {
    candidates.push({ to: match[1] as Domain, offset: match.index });
  }

  const workspaceCratePath = /\bcclover_(core|presentation|tui|desktop_ui|desktop)\s*::/g;
  while ((match = workspaceCratePath.exec(code))) {
    const workspaceDomain = match[1] === "desktop_ui" ? "ui" : match[1];
    candidates.push({ to: workspaceDomain as Domain, offset: match.index });
  }

  const superPath = /\b((?:super\s*::\s*)+)(core|platform|presentation|ui)\b/g;
  while ((match = superPath.exec(code))) {
    const supers = (match[1].match(/super/g) ?? []).length;
    if (supers > module.length) continue;
    const base = module.slice(0, module.length - supers);
    if (base.length === 0) candidates.push({ to: match[2] as Domain, offset: match.index });
  }

  candidates.push(...bracedUseDependencies(code, module));

  const seen = new Set<string>();
  return candidates
    .filter(({ to }) => FORBIDDEN[from].has(to))
    .filter(({ to, offset }) => {
      const key = `${to}:${offset}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    })
    .map(({ to, offset }) => ({ file, line: lineAt(code, offset), from, to }));
}

function rustFiles(root: string): string[] {
  const files: string[] = [];
  for (const entry of readdirSync(root)) {
    const path = join(root, entry);
    const stat = statSync(path);
    if (stat.isDirectory()) files.push(...rustFiles(path));
    else if (entry.endsWith(".rs")) files.push(path);
  }
  return files;
}

export function scanArchitecture(root = resolve(import.meta.dir)): Violation[] {
  const roots = root.endsWith(`${sep}src`)
    ? [root]
    : [
        join(root, "src"),
        join(root, "crates", "cclover-core", "src"),
        join(root, "crates", "cclover-presentation", "src"),
        join(root, "crates", "cclover-tui", "src"),
        join(root, "crates", "cclover-desktop-ui", "src"),
        join(root, "crates", "cclover-desktop", "src"),
      ].filter(existsSync);
  return roots.flatMap((sourceRoot) =>
    rustFiles(sourceRoot).flatMap((file) => findViolationsInSource(readFileSync(file, "utf8"), file)),
  );
}

export function formatViolation(violation: Violation, cwd = import.meta.dir): string {
  const file = relative(cwd, violation.file) || basename(violation.file);
  return `${file}:${violation.line}: forbidden architecture dependency ${violation.from} -> ${violation.to}`;
}

if (import.meta.main) {
  const violations = scanArchitecture();
  if (violations.length === 0) {
    console.log("architecture gate: ok");
  } else {
    for (const violation of violations) console.error(formatViolation(violation));
    process.exitCode = 1;
  }
}
