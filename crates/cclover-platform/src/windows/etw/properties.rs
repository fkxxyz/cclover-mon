use super::*;

pub(super) struct PropertiesBuffer {
    words: Vec<u64>,
    bytes: usize,
}

impl PropertiesBuffer {
    pub(super) fn new(name: &[u16]) -> Self {
        let bytes = size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * size_of::<u16>();
        let words = bytes.div_ceil(size_of::<u64>());
        let mut this = Self {
            words: vec![0_u64; words],
            bytes,
        };
        this.reset(name);
        this
    }

    pub(super) fn reset(&mut self, name: &[u16]) {
        self.words.fill(0);
        let properties = self.as_mut_ptr();
        // SAFETY: buffer is aligned and large enough for EVENT_TRACE_PROPERTIES plus name.
        unsafe {
            (*properties).Wnode.BufferSize = self.bytes as u32;
            (*properties).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
            (*properties).Wnode.Guid = SESSION_GUID;
            (*properties).Wnode.ClientContext = 1;
            (*properties).LoggerNameOffset = size_of::<EVENT_TRACE_PROPERTIES>() as u32;
            let dst = (properties as *mut u8)
                .add((*properties).LoggerNameOffset as usize)
                .cast::<u16>();
            std::ptr::copy_nonoverlapping(name.as_ptr(), dst, name.len());
        }
    }

    pub(super) fn configure_for_start(&mut self) {
        let properties = self.as_mut_ptr();
        // SAFETY: properties points to initialized EVENT_TRACE_PROPERTIES in our owned buffer.
        unsafe {
            (*properties).LogFileMode = EVENT_TRACE_REAL_TIME_MODE | EVENT_TRACE_SYSTEM_LOGGER_MODE;
            (*properties).EnableFlags = EVENT_TRACE_FLAG_THREAD
                | EVENT_TRACE_FLAG_DISK_FILE_IO
                | EVENT_TRACE_FLAG_FILE_IO
                | EVENT_TRACE_FLAG_FILE_IO_INIT;
            (*properties).BufferSize = 64;
            (*properties).MinimumBuffers = 4;
            (*properties).MaximumBuffers = 32;
            (*properties).FlushTimer = 1;
        }
    }

    pub(super) fn as_mut_ptr(&mut self) -> *mut EVENT_TRACE_PROPERTIES {
        self.words.as_mut_ptr().cast()
    }
}

pub(super) fn wide_z(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

pub(super) fn guid_eq(left: &GUID, right: &GUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}
