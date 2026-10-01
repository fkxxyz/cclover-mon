define_native_abi! {
    structs {
        NativeText => CcloverText {
            ptr: const_u8_ptr,
            len: usize,
        }

        NativeFrame => CcloverTuiFrame {
            lines: const_native_text_ptr,
            line_count: usize,
        }
    }
}
