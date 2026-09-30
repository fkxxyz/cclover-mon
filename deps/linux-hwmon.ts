export const LINUX_HWMON = {
  remote: "https://github.com/torvalds/linux.git",
  ref: "v6.11",
  commit: "98f7e32f20d28ec452afb208f9cffc08448a2652",
  files: [
    { path: "drivers/hwmon/coretemp.c", sha256: "19d0b8462943fd4ca6abedcc7aebe68279f6d14678aecd8d6d41e91cb0e1c8ca" },
    { path: "drivers/hwmon/k10temp.c", sha256: "9bf20a81146434dacdab4f7a0460febccd402864d0513c2e978776c5985d17f7" },
    { path: "drivers/hwmon/k8temp.c", sha256: "5b7e3fd1fad55b23e50ac537ab5ee814b33535b0f82418d80f2030291bf7b592" },
    { path: "drivers/hwmon/nct6775-core.c", sha256: "d5562c815a4579035ab3d958877d01db1308e65fdef7f74bf6439e4af8b19b99" },
    { path: "drivers/hwmon/nct6775.h", sha256: "a482931f33bf8224f840b3b50b840ecad506a449e0b3ab50153c3ea3ab6563f4" },
  ],
} as const;
