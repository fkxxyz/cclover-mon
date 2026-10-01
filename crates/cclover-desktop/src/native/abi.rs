use std::ffi::c_void;

pub(crate) type MeasureTextFn =
    unsafe extern "C" fn(*mut c_void, *const u8, usize, u32, u32) -> f32;

macro_rules! abi_rust_type {
    (u32) => { u32 };
    (u64) => { u64 };
    (f32) => { f32 };
    (usize) => { usize };
    (const_u8_ptr) => { *const u8 };
    (const_command_ptr) => { *const NativeCommand };
    (const_point_ptr) => { *const NativePoint };
    (const_damage_rect_ptr) => { *const NativeDamageRect };
    (state_fn) => { unsafe extern "C" fn(*mut c_void) -> u32 };
    (scene_fn) => {
        unsafe extern "C" fn(*mut c_void, *mut c_void, MeasureTextFn, *mut SceneView)
    };
}

macro_rules! define_native_abi {
    (
        constants {
            $( $rust_const:ident => $c_const:ident = $value:expr; )*
        }
        structs {
            $(
                $rust_struct:ident => $c_struct:ident {
                    $( $field:ident : $field_type:ident, )*
                }
            )*
        }
    ) => {
        $( pub(crate) const $rust_const: u32 = $value; )*

        $(
            #[repr(C)]
            #[derive(Clone, Copy)]
            pub(crate) struct $rust_struct {
                $( pub(crate) $field: abi_rust_type!($field_type), )*
            }
        )*
    };
}

include!("../native_abi_spec.rs");
