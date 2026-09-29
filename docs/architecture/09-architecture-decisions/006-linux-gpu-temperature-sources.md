---
summary: "Chooses amdgpu sysfs/hwmon plus optional dynamically loaded NVML as Linux GPU telemetry sources behind one platform-neutral GPU metric."
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

# ADR 006: Linux GPU Telemetry Sources

## Decision

Linux exposes one platform-neutral `GpuSnapshot` per GPU. The shared model contains optional utilization, VRAM used/total, temperature, power, core clock, fan percent, and fan RPM fields. Unsupported fields remain `None`; collectors never fabricate zero to represent missing capability.

Linux collection combines two peer native sources:

```text
Linux GPU metric
  ├── AMD amdgpu
  │     ├── DRM/sysfs: identity, utilization, VRAM, core clock
  │     └── device hwmon: power, fan
  └── NVIDIA adapter
        └── dynamically loaded NVML: identity + supported telemetry
             ↓
       one core-owned GPU snapshot sequence keyed by GpuId
```

NVIDIA proprietary-driver telemetry uses NVML directly in-process. The adapter dynamically loads `libnvidia-ml.so.1`, initializes one session, enumerates devices once, retains reusable handles, and reads all supported fields from those handles. Production code does not invoke `nvidia-smi`.

AMD collection is capability-based. DRM card discovery identifies `amdgpu` devices, derives `GpuId` from canonical device identity, then reads only native files actually present. Current sources are `gpu_busy_percent`, `mem_info_vram_used`, `mem_info_vram_total`, active `pp_dpm_sclk`, and device-associated hwmon power/fan files; temperature arrives through generic hwmon reconciliation.

GPU temperature belongs to the GPU snapshot so current value and bounded history use the same `GpuId` as utilization and VRAM. Generic hwmon discovery remains vendor-neutral and emits temperature observations with physical-device identity where available. At batch composition, temperatures matching an actually discovered GPU are consumed into that GPU snapshot when no vendor-provided GPU temperature already exists; unmatched sensors remain generic. This avoids duplicate presentation without embedding GPU-vendor ownership rules in generic hwmon discovery.

## Rationale

The product semantic is a GPU, not a set of vendor-specific telemetry widgets. One typed snapshot lets presentation bind each current value directly to its corresponding history while keeping NVML/sysfs/hwmon details below the platform boundary.

Stable identity must be separate from display labels because labels and enumeration positions are neither unique nor durable enough for cross-sample history. NVIDIA uses stable NVML identity such as UUID; AMD uses canonical DRM device identity.

Dynamic NVML loading keeps NVIDIA support optional. Native sysfs/hwmon access keeps AMD support dependency-free. Both paths avoid subprocess creation and text protocol layers around native telemetry.

## Consequences

- Core, presentation, desktop, TUI, and Web consume one `GpuSnapshot` contract without vendor branching.
- Utilization, VRAM, and GPU temperature histories are independently bounded and keyed by the same `GpuId`.
- Power, core clock, and fan are current-state fields; no history is retained for them.
- A field missing on one device degrades only that capability; other fields for the device remain usable.
- Fan percent and RPM are distinct optional source semantics. Presentation prefers percent and falls back to RPM.
- NVML initialization and device handles are long-lived collector state; normal one-second sampling does not recreate the session.
- AMD paths are capability-probed rather than assumed from a GPU model whitelist.
- Windows remains free to use Windows-native GPU telemetry behind the same core contract.

## Rejected Alternatives

### Vendor-specific core/UI GPU models

Rejected because vendor distinction is an acquisition concern. It would duplicate history, formatting, and layout behavior.

### `nvidia-smi` or other subprocess polling

Rejected because it adds process creation and text parsing to a one-second sampling loop while NVML provides the native API directly.

### Mandatory link-time NVML dependency

Rejected because systems without NVIDIA's library must still start normally.

### Treating missing metrics as zero

Rejected because zero is valid telemetry. Absence must remain distinguishable from an observed zero value.

### Treating display labels or DRM card numbers as identity

Rejected because both can change independently of the physical device and would corrupt history association.

## Reassessment

Revisit if supported Linux drivers expose a more stable vendor-neutral GPU telemetry ABI covering the required fields, if NVML stops providing the required read-only telemetry, or if measured native polling overhead materially exceeds the current budget.
