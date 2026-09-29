export const LIBRE_HARDWARE_MONITOR = {
  repository: "https://github.com/LibreHardwareMonitor/LibreHardwareMonitor.git",
  reviewedCommit: "677a3a56abde9adff5abdb42db3a7638c9137572",
  license: "MPL-2.0",
  relevantSources: [
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/Chip.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/LpcIO.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/Nct677X.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/IT87XX.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/F718XX.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/Lpc/W836XX.cs",
    "LibreHardwareMonitorLib/Hardware/Motherboard/SuperIOHardware.cs",
  ],
} as const;
