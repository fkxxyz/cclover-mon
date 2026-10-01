#[cfg(test)]
mod tests {
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;

    use super::*;

    struct FakeEnvironment {
        build: Option<u32>,
        versions: RefCell<VecDeque<Option<Version>>>,
        device_present: bool,
        #[cfg(target_arch = "x86_64")]
        helper_result: Result<u32, i32>,
        helper_calls: Cell<u32>,
    }

    impl FakeEnvironment {
        fn new(versions: impl IntoIterator<Item = Option<Version>>) -> Self {
            Self {
                build: Some(minimum_windows_build()),
                versions: RefCell::new(versions.into_iter().collect()),
                device_present: false,
                #[cfg(target_arch = "x86_64")]
                helper_result: Ok(EXIT_READY),
                helper_calls: Cell::new(0),
            }
        }
    }

    impl ProvisionEnvironment for FakeEnvironment {
        fn windows_build(&self) -> Option<u32> {
            self.build
        }

        fn installed_driver_version(&self) -> Option<Version> {
            let mut values = self.versions.borrow_mut();
            if values.len() > 1 {
                values.pop_front().flatten()
            } else {
                values.front().copied().flatten()
            }
        }

        fn pawnio_device_present(&self) -> bool {
            self.device_present
        }

        #[cfg(target_arch = "x86_64")]
        fn launch_elevated_helper(&self) -> io::Result<u32> {
            self.helper_calls.set(self.helper_calls.get() + 1);
            self.helper_result.map_err(io::Error::from_raw_os_error)
        }
    }

    #[test]
    fn versions_compare_numerically_and_allow_newer_compatible_driver() {
        assert!(parse_version("2.2.1.0") > parse_version("2.2.0"));
        assert_eq!(parse_version("2.2.0"), Some(Version([2, 2, 0, 0])));
        assert_eq!(parse_version("2.2.0.0.1"), None);
    }

    #[test]
    fn unsupported_windows_never_attempts_provisioning() {
        let mut environment = FakeEnvironment::new([None]);
        environment.build = Some(minimum_windows_build() - 1);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Unsupported
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    fn newer_driver_is_reused_without_downgrade_or_elevation() {
        let environment = FakeEnvironment::new([Some(Version([9, 0, 0, 0]))]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Ready
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    fn unknown_preexisting_device_is_not_overwritten() {
        let mut environment = FakeEnvironment::new([None]);
        environment.device_present = true;

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::InstallFailed
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn concurrent_install_is_rechecked_after_elevated_helper() {
        let required = required_driver_version();
        let environment = FakeEnvironment::new([None, None, Some(required)]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::Ready
        );
        assert_eq!(environment.helper_calls.get(), 1);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn uac_cancellation_is_a_stable_typed_state() {
        let mut environment = FakeEnvironment::new([None]);
        environment.helper_result = Err(ERROR_CANCELLED as i32);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::ElevationDeclined
        );
        assert_eq!(environment.helper_calls.get(), 1);
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn reboot_required_is_preserved() {
        let mut environment = FakeEnvironment::new([None]);
        environment.helper_result = Ok(EXIT_REBOOT_REQUIRED);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::RebootRequired
        );
    }

    #[cfg(not(target_arch = "x86_64"))]
    #[test]
    fn x86_process_does_not_attempt_driver_provisioning() {
        let environment = FakeEnvironment::new([None]);

        assert_eq!(
            prepare_machine_capability_with(&environment),
            MachineStatus::ProvisioningUnsupported
        );
        assert_eq!(environment.helper_calls.get(), 0);
    }

    #[test]
    #[cfg(target_arch = "x86_64")]
    fn multi_string_parsing_stops_at_double_nul() {
        let value = wide_multi_z("Root\\PawnIO");
        assert_eq!(multi_sz(&value).collect::<Vec<_>>(), ["Root\\PawnIO"]);
    }
}
