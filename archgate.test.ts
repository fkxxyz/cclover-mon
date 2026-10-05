import { describe, expect, test } from "bun:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

import {
  DOMAIN_NAMES,
  DOMAIN_RULES,
  domainForWorkspaceCrate,
  findNativeTextualIncludeViolationsInSource,
  findViolationsInSource,
  scanArchitecture,
  stripRustNonCode,
} from "./archgate";

const cases: Array<[string, string, string, number]> = [
  ["core -> platform", "src/core/model.rs", "use crate::platform::Backend;", 1],
  ["core -> presentation", "src/core/model.rs", "crate::presentation::Dashboard::default();", 1],
  ["core -> ui", "src/core/model.rs", "pub use crate::ui::PanelLayout;", 1],
  ["core -> tui", "src/core/model.rs", "use crate::tui::Terminal;", 1],
  ["workspace core -> platform", "crates/cclover-core/src/model.rs", "use crate::platform::Backend;", 1],
  ["workspace core -> presentation crate", "crates/cclover-core/src/model.rs", "use cclover_presentation::Dashboard;", 1],
  ["workspace presentation -> core crate allowed", "crates/cclover-presentation/src/lib.rs", "use cclover_core::model::MonitorState;", 0],
  ["workspace presentation -> platform", "crates/cclover-presentation/src/lib.rs", "use crate::platform::Backend;", 1],
  ["platform -> core allowed", "crates/cclover-platform/src/linux/mod.rs", "use cclover_core::Collector;", 0],
  ["platform -> presentation", "crates/cclover-platform/src/linux/mod.rs", "use cclover_presentation::Dashboard;", 1],
  ["platform -> ui", "crates/cclover-platform/src/linux/mod.rs", "use cclover_ui::PanelLayout;", 1],
  ["presentation -> core allowed", "src/presentation.rs", "use crate::core::model::MonitorState;", 0],
  ["presentation -> platform", "src/presentation.rs", "use crate::platform::Backend;", 1],
  ["presentation -> ui", "src/presentation.rs", "crate::ui::render();", 1],
  ["ui -> presentation allowed", "src/ui/mod.rs", "use crate::presentation::Dashboard;", 0],
  ["ui -> core allowed", "src/ui/layout.rs", "use crate::core::model::MonitorState;", 0],
  ["ui -> platform", "src/ui/layout.rs", "use crate::platform::Backend;", 1],
  ["tui -> presentation allowed", "src/tui.rs", "use crate::presentation::Dashboard;", 0],
  ["workspace tui -> presentation crate allowed", "crates/cclover-tui/src/lib.rs", "use cclover_presentation::Dashboard;", 0],
  ["workspace tui -> core crate", "crates/cclover-tui/src/lib.rs", "use cclover_core::Sampler;", 1],
  ["workspace web ui -> presentation allowed", "crates/cclover-web-ui/src/lib.rs", "use cclover_presentation::Dashboard;", 0],
  ["workspace web ui -> platform", "crates/cclover-web-ui/src/lib.rs", "use crate::platform::Backend;", 1],
  ["workspace core -> web ui crate", "crates/cclover-core/src/model.rs", "use cclover_web_ui::Panel;", 1],
  ["workspace core -> desktop crate", "crates/cclover-core/src/model.rs", "use cclover_desktop::DesktopApp;", 1],
  ["workspace crate alias", "crates/cclover-core/src/model.rs", "use cclover_desktop as desktop;", 1],
  ["workspace desktop -> presentation allowed", "crates/cclover-desktop/src/native.rs", "use cclover_presentation::Dashboard;", 0],
  ["workspace desktop -> platform", "crates/cclover-desktop/src/native.rs", "use crate::platform::Backend;", 1],
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

  test("every domain pair follows the declarative policy", () => {
    const sourceFiles = {
      core: "src/core/model.rs",
      platform: "crates/cclover-platform/src/linux/mod.rs",
      presentation: "src/presentation.rs",
      ui: "src/ui/layout.rs",
      tui: "src/tui.rs",
      runtime: "crates/cclover-runtime/src/lib.rs",
      http: "crates/cclover-http/src/lib.rs",
      server_app: "crates/cclover-server/src/main.rs",
    } as const;

    for (const from of DOMAIN_NAMES) {
      for (const to of DOMAIN_NAMES) {
        const violations = findViolationsInSource(`use crate::${to}::Marker;`, sourceFiles[from]);
        const expected = DOMAIN_RULES[from].forbidden.includes(to) ? 1 : 0;
        expect(violations, `${from} -> ${to}`).toHaveLength(expected);
      }
    }
  });

  test("every workspace crate alias resolves through the same domain registry", () => {
    for (const domain of DOMAIN_NAMES) {
      for (const crateName of DOMAIN_RULES[domain].workspaceCrates) {
        expect(domainForWorkspaceCrate(crateName.replaceAll("-", "_"))).toBe(domain);
      }
    }
  });
});

describe("native host implementation boundaries", () => {
  test("textual implementation includes are rejected", () => {
    const source = `#include "render.h"\n#include "legacy.inc"\n#include <other.c>\n`;
    expect(findNativeTextualIncludeViolationsInSource(source, "native/linux_host.c")).toEqual([
      expect.objectContaining({ line: 2, includePath: "legacy.inc" }),
      expect.objectContaining({ line: 3, includePath: "other.c" }),
    ]);
  });

  test("headers remain valid dependencies", () => {
    expect(
      findNativeTextualIncludeViolationsInSource(
        `#include "render.h"\n#include <windows.h>\n`,
        "native/linux/render.c",
      ),
    ).toEqual([]);
  });

  test("generated Wayland protocol sources are excluded without exempting project-owned neighbors", () => {
    const root = mkdtempSync(join(tmpdir(), "cclover-archgate-native-"));
    try {
      const wayland = join(root, "crates", "cclover-desktop", "native", "wayland");
      mkdirSync(wayland, { recursive: true });
      writeFileSync(
        join(wayland, "wlr-layer-shell-unstable-v1-protocol.c"),
        `#include "generated.inc"\n`,
      );
      writeFileSync(join(wayland, "project_owned.c"), `#include "forbidden.inc"\n`);

      expect(scanArchitecture(root)).toEqual([
        expect.objectContaining({
          line: 1,
          rule: "native-textual-implementation-include",
          includePath: "forbidden.inc",
        }),
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
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
    expect(scanArchitecture(resolve(import.meta.dir))).toEqual([]);
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

  test("registered workspace crate roots are discovered from the domain registry", () => {
    const root = mkdtempSync(join(tmpdir(), "cclover-archgate-workspace-"));
    try {
      const core = join(root, "crates", "cclover-core", "src");
      mkdirSync(core, { recursive: true });
      writeFileSync(join(core, "lib.rs"), "use cclover_desktop as desktop;\n");
      expect(scanArchitecture(root)).toEqual([
        expect.objectContaining({ line: 1, from: "core", to: "ui" }),
      ]);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
});
