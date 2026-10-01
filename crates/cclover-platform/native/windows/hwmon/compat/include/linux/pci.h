#ifndef CCLOVER_LINUX_PCI_H
#define CCLOVER_LINUX_PCI_H
#include <linux/types.h>
struct kobject { int unused; };
struct device { void *drvdata; struct kobject kobj; struct device *parent; };
struct pci_bus { unsigned int number; };
struct pci_dev {
    struct device dev;
    struct pci_bus *bus;
    unsigned int devfn;
    u16 vendor;
    u16 device;
};
struct pci_device_id { u32 vendor; u32 device; };
struct pci_driver { const char *name; const struct pci_device_id *id_table; int (*probe)(struct pci_dev *, const struct pci_device_id *); };
#define PCI_DEVFN(slot, func) ((((slot) & 0x1f) << 3) | ((func) & 0x07))
#define PCI_SLOT(devfn) (((devfn) >> 3) & 0x1f)
#define PCI_VENDOR_ID_INTEL 0x8086
#define PCI_VENDOR_ID_AMD 0x1022
#define PCI_VENDOR_ID_HYGON 0x1d94
#define PCI_DEVICE_ID_AMD_K8_NB_MISC 0x1103
#define PCI_VDEVICE(vendor_name, device_id) .vendor = PCI_VENDOR_ID_##vendor_name, .device = (device_id)
#define PCI_DEVICE(vendor_id, device_id) .vendor = (vendor_id), .device = (device_id)
#define to_pci_dev(d) container_of((d), struct pci_dev, dev)
int pci_read_config_byte(struct pci_dev *pdev, int where, u8 *value);
int pci_write_config_byte(struct pci_dev *pdev, int where, u8 value);
int pci_read_config_dword(struct pci_dev *pdev, int where, u32 *value);
int pci_bus_read_config_dword(struct pci_bus *bus, unsigned int devfn, int where, u32 *value);
int pci_bus_write_config_dword(struct pci_bus *bus, unsigned int devfn, int where, u32 value);
struct pci_dev *pci_get_domain_bus_and_slot(int domain, unsigned int bus, unsigned int devfn);
void pci_dev_put(struct pci_dev *pdev);
static inline void *dev_get_drvdata(struct device *dev) { return dev->drvdata; }
static inline void dev_set_drvdata(struct device *dev, void *data) { dev->drvdata = data; }
#endif
