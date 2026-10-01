use super::*;

pub(super) trait ProvisionEnvironment {
    fn windows_build(&self) -> Option<u32>;
    fn installed_driver_version(&self) -> Option<Version>;
    fn pawnio_device_present(&self) -> bool;
    #[cfg(target_arch = "x86_64")]
    fn launch_elevated_helper(&self) -> io::Result<u32>;
}

pub(super) struct SystemEnvironment;

impl ProvisionEnvironment for SystemEnvironment {
    fn windows_build(&self) -> Option<u32> {
        windows_build()
    }

    fn installed_driver_version(&self) -> Option<Version> {
        installed_driver_version()
    }

    fn pawnio_device_present(&self) -> bool {
        pawnio_device_present()
    }

    #[cfg(target_arch = "x86_64")]
    fn launch_elevated_helper(&self) -> io::Result<u32> {
        launch_elevated_helper()
    }
}

pub(super) fn prepare_machine_capability_with(
    environment: &impl ProvisionEnvironment,
) -> MachineStatus {
    if environment
        .windows_build()
        .is_none_or(|build| build < minimum_windows_build())
    {
        return MachineStatus::Unsupported;
    }

    let required = required_driver_version();
    if environment
        .installed_driver_version()
        .is_some_and(|installed| installed >= required)
    {
        return MachineStatus::Ready;
    }
    if environment.installed_driver_version().is_none() && environment.pawnio_device_present() {
        return MachineStatus::InstallFailed;
    }

    #[cfg(not(target_arch = "x86_64"))]
    {
        return MachineStatus::ProvisioningUnsupported;
    }
    #[cfg(target_arch = "x86_64")]
    {
        match environment.launch_elevated_helper() {
            Ok(EXIT_READY) => {
                if environment
                    .installed_driver_version()
                    .is_some_and(|installed| installed >= required)
                {
                    MachineStatus::Ready
                } else {
                    MachineStatus::InstallFailed
                }
            }
            Ok(EXIT_REBOOT_REQUIRED) => MachineStatus::RebootRequired,
            Err(error) if error.raw_os_error() == Some(ERROR_CANCELLED as i32) => {
                MachineStatus::ElevationDeclined
            }
            _ => MachineStatus::InstallFailed,
        }
    }
}
