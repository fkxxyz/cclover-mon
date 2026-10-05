export const PAWNIO = {
  driver: {
    id: "pawnio-driver",
    version: "2.2.0",
    installerUrl:
      "https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe",
    installerSha256: "1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032",
    embeddedCabSha256: "c2f74446ddebabeeaa8a5fe36c3be3622edb4fac0635d81abba331a817f432b0",
    correspondingSource: {
      repository: "https://github.com/namazso/PawnIO.git",
      ref: "2.2.0",
      commit: "5cdf470831fdfff3f7f1d06363ca6b230f3bf35a",
      browseUrl: "https://github.com/namazso/PawnIO/tree/2.2.0",
    },
    redistribution: {
      license: "GPL-2.0-or-later with the upstream PawnIO special exception",
      noticeFiles: ["LICENSES/PawnIO-NOTICE.txt"],
      fulfillment: "companion-source",
    },
    minWindowsBuild: 17763,
    targets: {
      x86_64: {
        runtimeSupported: true,
        provisioningSupported: true,
        driverArchitecture: "amd64",
      },
      x86: {
        runtimeSupported: true,
        provisioningSupported: false,
        driverArchitecture: "amd64",
      },
    },
    productionAmd64: {
      inf: {
        name: "PawnIO.inf",
        sha256: "7c1c203e13693531243fbee3cb87d7b79170eae89f5729b3f41387fe68a54f0b",
      },
      sys: {
        name: "PawnIO.sys",
        sha256: "fca6e7d58b0cf38dbb913a2b9e532f48629145d395f454b16a9f58e97b8d3940",
      },
      cat: {
        name: "PawnIO.cat",
        sha256: "a37d46840280efec92063d3a21014c803939e599b4c0da4a4d063b79eeca9446",
      },
    },
  },
  modules: {
    id: "pawnio-modules",
    version: "0.2.10",
    archiveUrl:
      "https://github.com/namazso/PawnIO.Modules/releases/download/0.2.10/release_0_2_10.zip",
    archiveSha256: "971c7c974c538b62ac020e0442fa99d0423417bfb496dfe9a4a43ccc0abc0e63",
    correspondingSource: {
      repository: "https://github.com/namazso/PawnIO.Modules.git",
      ref: "0.2.10",
      commit: "c683032770575d7705d1149f9d7fa7fd381766fc",
      browseUrl: "https://github.com/namazso/PawnIO.Modules/tree/0.2.10",
    },
    redistribution: {
      license: "LGPL-2.1-or-later",
      noticeFiles: ["LICENSES/LGPL-2.1-or-later.txt"],
      fulfillment: "companion-source",
    },
    required: {
      intelMsr: {
        name: "IntelMSR.bin",
        sha256: "d6ed85d65ab17a22f813ef98207d6d537155ee2ded5976a21cb48413c9b92e5f",
      },
      amdFamily0f: {
        name: "AMDFamily0F.bin",
        sha256: "a6e11619e87a97820705a6523714f22d676ce44f902631833d4429b89d509d55",
        targets: ["x86_64"],
      },
      amdFamily10: {
        name: "AMDFamily10.bin",
        sha256: "6443080b2968474ffbc38aa4356cc56f9664349fa4b917afdb33027d6bb50525",
        targets: ["x86_64"],
      },
      amdFamily17: {
        name: "AMDFamily17.bin",
        sha256: "dae74615761b78bdf064dfb3e136252ddcc6fc727d88f14738d0e5800d427a91",
        targets: ["x86_64"],
      },
      lpcAcpiEc: {
        name: "LpcACPIEC.bin",
        sha256: "c38fd116e7aff4d1fdb0a494e296be0a6708e5a22fc72f14587442fb7f8f7906",
        targets: ["x86_64"],
      },
      lpcCrosEc: {
        name: "LpcCrOSEC.bin",
        sha256: "1ca4b495ea09dc05278e8627bda993e20e8d66a9f90e866283f0007ef0d57e28",
        targets: ["x86_64"],
      },
      lpcIo: {
        name: "LpcIO.bin",
        sha256: "b3896a1cab0d808fca31fe2ebcae045d59dac690da87b17c858bb8da357eb45e",
        targets: ["x86_64"],
      },
    },
  },
} as const;

export type PawnioPayloadId = typeof PAWNIO.driver.id | typeof PAWNIO.modules.id;
