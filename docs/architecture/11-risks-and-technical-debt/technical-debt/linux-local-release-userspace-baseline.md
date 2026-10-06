---
summary: "Records that local and self-hosted Linux release builds can still derive native-library API requirements from an uncontrolled host userspace."
viewpoint: assurance
concerns:
  - portability
  - maintainability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - whole-system
---

# Linux Local Release Userspace Baseline

**Priority:** Low-medium

## Root cause

The official GitHub Linux release path uses the declared Ubuntu 22.04 userspace generation, but local and self-hosted `release.ts build` executions still compile and link against the native libraries supplied by their host. Final-ELF policy can bound the loader, GLIBC/GCC symbol generations, libbpf ABI, and direct SONAME set, but it cannot prove that imports from Cairo, X11, Wayland, GLib, or another library with no project-governed symbol-version ceiling exist in the Ubuntu 22.04 userspace generation.

## Primary cost dimension

Portability and release reliability.

## Current cost

A maintainer can produce a locally packaged Linux artifact that passes the repository's final-ELF checks while using a native-library API newer than the official hosted baseline. Such an artifact is useful for local inspection, but it cannot carry the same compatibility evidence as an artifact built by the controlled hosted path. The distinction is currently implicit in build location rather than mechanically represented by release authority.

## Evidence

The current work established that official hosted Linux build jobs are pinned to Ubuntu 22.04 and that final artifacts enforce loader, GLIBC/GCC, libbpf, and mandatory SONAME policy. The same `release.ts build` command also succeeds on the current Arch Linux development host. ELF inspection cannot generally infer the minimum Cairo/X11/Wayland/GLib package generation required by arbitrary imported symbols.

## Cost mechanism

Host-native development libraries remain an implicit partial compatibility authority outside the hosted release environment. Because those libraries can advance independently of product policy, local or self-hosted packaged artifacts can satisfy the mechanically visible ELF ceilings while exceeding the intended non-versioned native userspace baseline.

## Reachable better state

Give Linux release builds one controlled userspace/sysroot authority independent of the invoking host, or explicitly distinguish uncontrolled-host packages from artifacts that are eligible to claim the official compatibility baseline. A pinned container, sysroot, or equivalent verified build environment is acceptable if it is owned by release policy and does not create a second independently maintained dependency matrix.

## Governing constraint

Any Linux artifact represented as satisfying the official release compatibility baseline must be compiled and linked against a userspace whose native-library generation is mechanically tied to that baseline; host-native library drift must not silently supply compatibility evidence.

## Scope discovery

Inspect every path that can invoke Linux `release.ts build`, including local maintenance, self-hosted automation, and hosted CI. Inspect all native dynamic dependencies whose API generation is not completely bounded by the existing ELF symbol-version checks. Distinguish packaging useful for developer inspection from artifacts eligible for official publication or compatibility claims.

## Repair direction

Prefer one controlled Linux release build environment or sysroot consumed by all compatibility-claiming build paths. Avoid constructing a manual per-library symbol database or a distro compatibility matrix. If local uncontrolled builds remain useful, make their weaker compatibility evidence explicit rather than forbidding them unnecessarily.

## Exit criteria

- Every Linux artifact eligible to claim the official compatibility baseline is built against a userspace mechanically tied to that baseline.
- Local or self-hosted host-library drift cannot silently produce an artifact with equivalent compatibility authority.
- Cairo, X11, Wayland, GLib, and other native dependencies are covered by the controlled userspace boundary without a manually maintained symbol matrix.
- The official hosted path and any equivalent local/self-hosted path consume one release-build environment authority rather than duplicated environment definitions.
