#ifndef CCLOVER_ASM_AMD_NB_H
#define CCLOVER_ASM_AMD_NB_H
#include <linux/pci.h>
int amd_smn_read(u16 node, u32 address, u32 *value);
u16 amd_pci_dev_to_node_id(struct pci_dev *pdev);
#endif
