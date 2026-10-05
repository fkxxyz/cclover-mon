---
summary: "Records that Linux libbpf API compatibility and optional-acceleration fallback lack one explicit authority and representative runtime proof."
viewpoint: assurance
concerns:
  - maintainability
  - portability
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# Linux libbpf Compatibility Authority

**Priority:** Low-medium

## Root cause

Linux eBPF attribution has architecture guidance for kernel/BTF/attach compatibility, but the repository does not define one explicit userspace libbpf compatibility authority: the minimum supported libbpf API baseline, which newer APIs may be treated as optional accelerations, and what fallback proof is required when they are optional. Each use of a newer libbpf entry point therefore requires local compatibility judgment.

## Primary cost dimension

Maintainability and Linux distribution portability.

## Current cost

Maintainers adding or upgrading libbpf calls must determine ad hoc whether directly linking a symbol is acceptable, whether runtime symbol resolution is required, which kernel/map errors mean "unsupported", and how to prove the legacy path still works. The burden is small for one API but compounds as optional libbpf features accumulate, while an incorrect decision can turn an optimization into a process-load failure or silently drop attribution support on older environments.

## Evidence

The eBPF map-batch optimization introduced `bpf_map_lookup_batch` as a userspace acceleration. Directly declaring the newer symbol would make the executable depend on that symbol at dynamic-load time, preventing any kernel-level fallback on systems whose installed `libbpf.so.1` lacks it. The implementation therefore had to resolve `bpf_map_lookup_batch` through `dlsym` and retain scalar map traversal when the symbol or kernel/map operation is unavailable.

The current development machine validates the supported batch path and the scalar policy through deterministic tests, but there is no representative runtime environment that proves the complete executable still performs attribution when the batch symbol is absent or when the kernel/map rejects batch lookup. The need for optional symbol resolution and the required fallback evidence were inferred during the change rather than consumed from a repository-owned compatibility contract.

## Cost mechanism

Without one compatibility authority, every newer libbpf API creates a fresh decision point across source linkage, runtime capability detection, error classification, deployment expectations, diagnostics, and validation. Local defensive branches can accumulate even when different APIs should follow the same compatibility rule, while maintainers cannot safely delete those branches because the supported baseline and retirement condition are unclear.

## Reachable better state

Define a small Linux/libbpf compatibility contract in the appropriate architecture/deployment authority. It should state the minimum supported userspace libbpf baseline, distinguish mandatory APIs from optional accelerations, require optional accelerations to preserve the established scalar/legacy behavior when unavailable, and define the cheapest sufficient runtime proof for that fallback. Keep API-specific mechanics inside the Linux platform boundary rather than creating a general compatibility framework.

## Governing constraint

Adding an optional libbpf acceleration must not raise the effective runtime libbpf requirement or remove an established attribution capability unless that compatibility change is explicit in the authoritative deployment contract and validated as such.

## Scope discovery

Review all dynamically linked libbpf entry points used by Linux attribution, the eBPF loader/map-access adapter, Linux deployment requirements, package/runtime dependency declarations, diagnostic failure classification, and Linux runtime validation. Identify every API whose availability can vary independently of the declared supported baseline, and distinguish kernel/BTF/attach uncertainty already tracked as architecture risk from avoidable userspace compatibility-policy gaps.

## Repair direction

Choose and document the supported libbpf baseline from actual deployment targets. For APIs newer than that baseline, either avoid them or treat them as optional accelerations behind one locally contained availability policy. Add the smallest executable proof that can run the production attribution path with the optional API unavailable or rejected; prefer a controlled shim/container/runtime fixture over copying collector behavior into a test implementation. Expose enough diagnostic information to distinguish optional-acceleration fallback from genuine attribution failure without adding normal-path logging noise.

Do not introduce a broad version-negotiation subsystem, duplicate libbpf feature matrices in multiple documents, or treat every kernel attach incompatibility as userspace library debt.

## Exit criteria

- One architecture/deployment authority states the minimum supported libbpf userspace baseline and the policy for APIs newer than that baseline.
- Every current libbpf call is classified as baseline-required or optional according to that authority.
- Optional map-batch acceleration is proven through the production attribution path when its symbol is absent or its operation is rejected, with scalar behavior remaining available.
- Diagnostics can distinguish an optional acceleration fallback from a genuine attribution capability failure when investigation requires it.
- Adding another optional libbpf API no longer requires inventing a separate compatibility policy or duplicating baseline facts.
