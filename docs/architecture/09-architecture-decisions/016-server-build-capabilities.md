---
summary: "Chooses build-time Cargo features for optional headless-server capabilities, starting with eBPF I/O attribution, and a self-contained static aarch64 musl build for minimal Linux devices."
viewpoint: decision
concerns:
  - portability
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
    - platform
---

# ADR 016: Server Build Capabilities and Static Linux Target

## Decision

Optional native capabilities of `cclover-mon-server` are selected at build time through Cargo features. The first and only such capability is `ebpf-io`:

- `cclover-server` declares `default = ["ebpf-io"]`, so existing builds keep process I/O attribution and their libbpf/libelf/zlib/zstd dependencies.
- `--no-default-features` builds a variant with no libbpf link, no clang BPF compilation, and no BTF requirement. Attribution metrics report `disabled`; every other collector is unaffected.
- The variant can be built as a self-contained `aarch64-unknown-linux-musl` executable with `--profile dist` and `rust-lld`, without a separate C cross-linker. `bun validate.ts server-static` verifies the result (README documents the build command).
- `--version` reports the build-time feature set. Runtime availability (BTF, capabilities, attach points) stays in published state and is not inferred from the build.

NVML is deliberately not a feature. It is already loaded with `dlopen` and has no link-time dependency, so a feature would only add a test matrix without changing packaging.

## Rationale

eBPF is the only server capability with build-time dependencies: clang for BPF objects, libbpf, libelf, zlib, zstd, and a C compiler for the target. It also needs BTF on the running kernel, which some supported devices lack. Removing it is therefore a packaging choice that belongs to the build. Keeping `ebpf-io` as the same feature name in `cclover-platform` and `cclover-server` keeps Cargo unification predictable.

Runtime `dlopen` of libbpf was rejected because a static musl executable cannot load shared objects, the BPF object still requires clang at build time, and a single binary carrying both paths would still need the full toolchain to produce it.

## Consequences

- The static aarch64 artifact is a validated build, not a published release. The release plan publishes only the artifacts it declares (`tools/release/plan.ts`); publishing this one requires an explicit plan change, and the release-ABI policy must then gain a static-artifact rule.
- The release plan has no capability model. Add one when a second optional capability or a second publishable variant exists, and not before.
- Every Rust FFI buffer that holds C strings uses `c_char`, not `i8`. On aarch64 `c_char` is unsigned, so the fixed `i8` declarations failed to compile there. The non-eBPF path is enforced by the `server-static` build; the eBPF path has no aarch64 proof (see Architecture Risks).
- Each server configuration must be validated: the `server` profile builds and smokes both the default and the no-eBPF variants, and `--version` must match the binary under test.

## Rejected Alternatives

- Runtime libbpf loading: see Rationale.
- A capability matrix in the release plan now: one capability does not justify the schema and verification cost.
- An NVML feature: no dependency or size benefit exists.
- Container packaging or zig-based cross compilation: the target runs directly on the host; `rust-lld` is sufficient.
