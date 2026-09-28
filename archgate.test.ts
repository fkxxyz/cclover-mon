import { describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import { findViolationsInSource, scanArchitecture, stripRustNonCode } from "./archgate";

const cases: Array<[string, string, string, number]> = [
  ["core -> platform", "src/core/model.rs", "use crate::platform::Backend;", 1],
  ["core -> presentation", "src/core/model.rs", "crate::presentation::Dashboard::default();", 1],
  ["core -> ui", "src/core/model.rs", "pub use crate::ui::PanelLayout;", 1],
  ["platform -> core allowed", "src/platform/linux/mod.rs", "use crate::core::Collector;", 0],
  ["platform -> presentation", "src/platform/linux/mod.rs", "use crate::presentation::Dashboard;", 1],
  ["platform -> ui", "src/platform/linux/mod.rs", "use crate::ui::PanelLayout;", 1],
  ["presentation -> core allowed", "src/presentation.rs", "use crate::core::model::MonitorState;", 0],
  ["presentation -> platform", "src/presentation.rs", "use crate::platform::Backend;", 1],
  ["presentation -> ui", "src/presentation.rs", "crate::ui::render();", 1],
  ["ui -> presentation allowed", "src/ui/mod.rs", "use crate::presentation::Dashboard;", 0],
  ["ui -> core allowed", "src/ui/layout.rs", "use crate::core::model::MonitorState;", 0],
  ["ui -> platform", "src/ui/layout.rs", "use crate::platform::Backend;", 1],
  ["tui -> presentation allowed", "src/tui.rs", "use crate::presentation::Dashboard;", 0],
  ["tui -> core", "src/tui.rs", "use crate::core::Sampler;", 1],
  ["tui -> platform", "src/tui.rs", "use crate::platform::Backend;", 1],
  ["tui -> ui", "src/tui.rs", "use crate::ui::PanelLayout;", 1],
  ["braced crate use", "src/core/model.rs", "use crate::{core::history, platform::Backend};", 1],
  ["resolved super path", "src/core/nested/item.rs", "use super::super::super::platform::Backend;", 1],
  ["resolved super braced use", "src/core/nested/item.rs", "use super::super::super::{platform::Backend, core::model};", 1],
];

describe("architecture dependency rules", () => {
  test.each(cases)("%s", (_name, file, source, expected) => {
    expect(findViolationsInSource(source, file)).toHaveLength(expected);
  });
});

describe("lexical filtering", () => {
  test("comments and strings cannot create dependencies", () => {
    const source = `
// use crate::platform::Backend;
/* crate::platform::Backend */
const A: &str = "crate::platform::Backend";
const B: &str = r#"use crate::{platform::Backend};"#;
use crate::core::model::MonitorState;
`;
    expect(findViolationsInSource(source, "src/core/model.rs")).toEqual([]);
  });

  test("nested block comments preserve following code and line numbers", () => {
    const source = `/* outer /* inner */ done */\nuse crate::platform::Backend;`;
    const [violation] = findViolationsInSource(source, "src/core/model.rs");
    expect(violation?.line).toBe(2);
  });

  test("stripping preserves source length", () => {
    const source = `// comment\nlet x = "text"; /* block */\n`;
    expect(stripRustNonCode(source)).toHaveLength(source.length);
  });
});

describe("repository gate", () => {
  test("current source tree satisfies architecture direction", () => {
    expect(scanArchitecture(resolve(import.meta.dir, "src"))).toEqual([]);
  });

  test("fixture violation fails with source location", () => {
    const root = mkdtempSync(join(tmpdir(), "cclover-archgate-"));
    try {
      const core = join(root, "src", "core");
      mkdirSync(core, { recursive: true });
      writeFileSync(join(core, "bad.rs"), "\nuse crate::platform::Backend;\n");
      expect(scanArchitecture(join(root, "src"))).toEqual([
        expect.objectContaining({ line: 2, from: "core", to: "platform" }),
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
