define_native_abi! {
    structs {
        NativeText => CcloverText {
            ptr: const_u8_ptr,
            len: usize,
        }

        NativePanel => CcloverPanel {
            title: native_text,
            rows: const_native_text_ptr,
            row_count: usize,
            history: const_u64_ptr,
            history_count: usize,
        }

        NativeFrame => CcloverTuiFrame {
            cpu: native_panel,
            memory: native_panel,
            gpu: native_panel,
            temperatures: native_panel,
            fans: native_panel,
            disks: native_panel,
            networks: native_panel,
        }
    }
}
