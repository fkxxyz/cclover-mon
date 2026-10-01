#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_buffer_keeps_session_name_after_header() {
        let name = wide_z("test-session");
        let mut buffer = PropertiesBuffer::new(&name);
        let properties = buffer.as_mut_ptr();
        // SAFETY: PropertiesBuffer owns the complete header + UTF-16 name allocation.
        let stored = unsafe {
            let ptr = (properties as *const u8)
                .add((*properties).LoggerNameOffset as usize)
                .cast::<u16>();
            std::slice::from_raw_parts(ptr, name.len())
        };
        assert_eq!(stored, name.as_slice());
    }
}
