---
summary: "Chooses hwmon plus optional dynamically loaded NVML as peer Linux GPU-temperature sources behind one platform-neutral temperature metric."
viewpoint: decision
concerns:
  - architecture-coherence
  - performance
  - maintainability
  - portability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# ADR 006: Linux GPU Temperature Sources

## Decision

Linux GPU temperatures remain part of the existing temperature metric rather than becoming a vendor-specific core or UI metric.

Linux temperature collection combines two peer native sources:

```text
Linux temperature metric
  ├── temperature collector → hwmon
  │     ├── AMD amdgpu temperatures
  │     ├── supported Intel i915/xe temperatures
  │     └── other kernel-exposed temperature sensors
  └── shared NVIDIA telemetry adapter → NVML
        └── every enumerated proprietary-driver NVIDIA GPU with readable temperature
             ↓
       one core-owned temperature snapshot sequence
```

hwmon remains the generic Linux kernel sensor path. NVIDIA proprietary-driver temperature telemetry uses NVML directly in-process. The shared NVIDIA adapter dynamically loads `libnvidia-ml.so.1`, initializes one session, enumerates all NVIDIA devices, retains reusable device handles, and serves temperature plus other NVIDIA telemetry such as GPU memory from that same session. Production code does not invoke `nvidia-smi`.

NVML is optional. Missing library, initialization failure, or zero enumerated NVIDIA devices contributes no NVML temperature entries and does not affect startup or hwmon collection. Failure to read one device temperature omits only that sensor. No zero value is fabricated for unavailable data.

Every temperature sensor crossing into core has a stable identity distinct from its display label. NVIDIA identity is derived from a stable NVML device identity such as UUID, while the display label comes from `nvmlDeviceGetName()` and remains independent from history identity. hwmon identity is likewise derived from stable platform/device/channel identity rather than the human-readable label. Core history keys use sensor identity. Presentation/UI consume the merged temperature sequence uniformly and do not branch on hwmon versus NVML. Presentation may remove known redundant vendor/product-family prefixes from NVIDIA labels without changing the core snapshot value.

## Rationale

AMD and supported Intel Linux drivers already expose GPU temperature through the kernel hwmon ABI, which is cheap, native, and vendor-neutral. NVIDIA's proprietary driver exposes supported telemetry through NVML; spawning `nvidia-smi` would only add a process lifecycle and text-parsing layer around the same management interface.

Keeping hwmon and NVML as peer sources under one metric preserves the existing platform boundary. It prevents native library types and vendor distinctions from leaking into core, history, presentation, or frontend layout. It also lets multiple GPUs appear as ordinary parallel temperature entries without creating vendor-specific UI paths.

Stable identity must be separate from display labels because multiple GPUs or sensors may share the same friendly name, labels may change for presentation reasons, and NVML device indices are not a durable cross-restart identity.

Dynamic loading keeps NVIDIA support optional. A Linux build remains usable on systems with no NVIDIA driver or GPU and does not acquire a mandatory loader dependency solely for telemetry.

## Consequences

- Linux temperature collection becomes an internal fan-in of hwmon and NVML sources, while NVML native lifetime may be shared with other NVIDIA telemetry metrics.
- The core temperature model/history must distinguish stable sensor identity from display label.
- NVIDIA multi-GPU systems produce one temperature entry per device that supports the queried temperature.
- NVIDIA labels are read once from NVML during initialization; presentation may shorten only known redundant prefixes. hwmon naming semantics remain owned by the hwmon source and are not coupled to NVML naming.
- NVML initialization, discovery, and handles are long-lived collector state; normal one-second sampling reads existing devices rather than recreating the session each cycle.
- NVIDIA topology changes may require an explicit refresh/recovery path, but they do not justify per-sample rediscovery.
- Missing or unusable NVML is a degradable source condition and must remain diagnosable without becoming a user-visible failure of unrelated metrics.
- Read-only NVIDIA temperature telemetry must not introduce a root/setuid requirement or a helper process.
- Windows remains free to use Windows-native GPU telemetry behind the same core temperature semantics; Linux hwmon/NVML APIs do not become shared dependencies.

## Rejected Alternatives

### `nvidia-smi` subprocess polling

Rejected because it adds process creation, text parsing, external-command behavior, and avoidable overhead to a one-second monitor loop while NVML provides the native API directly.

### Mandatory link-time NVML dependency

Rejected because systems without NVIDIA's library must still start and run normally. NVIDIA telemetry is optional capability, not a deployment prerequisite.

### Vendor-specific core/UI temperature models

Rejected because hwmon and NVML differ only in native acquisition. The product semantic is still a temperature sensor, and source-specific contracts would duplicate history, presentation, and layout behavior.

### Treating display label as sensor identity

Rejected because labels are not unique or stable enough for multi-GPU history. Identity and presentation naming have different responsibilities.

### Rediscovering all devices every sample

Rejected because normal topology is stable and the one-second path should reuse hwmon discovery state and NVML session/device handles. Device refresh belongs to lifecycle/recovery handling instead.

## Reassessment

Revisit this decision if NVIDIA exposes equivalent proprietary-driver temperature data through a stable kernel hwmon ABI on the supported fleet, if NVML no longer provides the required read-only telemetry, or if measured long-lived NVML integration cost is materially worse than another in-process native interface with the same semantics.
