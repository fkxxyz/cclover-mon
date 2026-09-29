export const PAWNIO = {
  driver: {
    version: "2.2.0",
    installerUrl:
      "https://github.com/namazso/PawnIO.Setup/releases/download/2.2.0/PawnIO_setup.exe",
    installerSha256: "1f519a22e47187f70a1379a48ca604981c4fcf694f4e65b734aaa74a9fba3032",
    embeddedCabSha256: "c2f74446ddebabeeaa8a5fe36c3be3622edb4fac0635d81abba331a817f432b0",
    sourceUrl: "https://github.com/namazso/PawnIO/tree/2.2.0",
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
    version: "0.2.10",
    archiveUrl:
      "https://github.com/namazso/PawnIO.Modules/releases/download/0.2.10/release_0_2_10.zip",
    archiveSha256: "971c7c974c538b62ac020e0442fa99d0423417bfb496dfe9a4a43ccc0abc0e63",
    sourceUrl: "https://github.com/namazso/PawnIO.Modules/tree/0.2.10",
    license: "LGPL-2.1-or-later",
    required: {
      intelMsr: {
        name: "IntelMSR.bin",
        sha256: "d6ed85d65ab17a22f813ef98207d6d537155ee2ded5976a21cb48413c9b92e5f",
      },
    },
  },
} as const;
