---
summary: "Defines Linux eBPF disk attribution semantics and runtime correlation."
viewpoint: dynamic
concerns:
  - architecture-coherence
  - maintainability
  - security
activities:
  - orient
  - change
  - assess
facets:
  area:
    - platform
---

# Disk Attribution

The product semantic is **process × storage device × read/write bytes**.

The implemented metric measures **logical successful file-I/O bytes**. eBPF attaches to `vfs_read` / `vfs_write` return paths, attributes returned bytes to the current TGID, reads the backing filesystem `s_dev` through CO-RE, and resolves that native device to the product block-device identity in userspace.

This is intentionally not physical media traffic. Block-layer execution can occur in writeback or kernel-worker context, so using `current` at final device submission would not reliably preserve the originating process. Observing process identity and logical I/O result in the same call context avoids that misattribution.

Native device identity stays inside the Linux collector until it is canonicalized to the same core-owned `DiskId` used by the corresponding disk snapshot.
