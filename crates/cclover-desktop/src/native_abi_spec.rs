define_native_abi! {
    constants {
        POLL_FRAME => CCLOVER_POLL_FRAME = 1;
        POLL_QUIT => CCLOVER_POLL_QUIT = 2;
        COMMAND_FILL_RECT => CCLOVER_CMD_FILL_RECT = 1;
        COMMAND_STROKE_RECT => CCLOVER_CMD_STROKE_RECT = 2;
        COMMAND_TEXT => CCLOVER_CMD_TEXT = 3;
        COMMAND_POLYLINE => CCLOVER_CMD_POLYLINE = 4;
        COMMAND_POLYGON => CCLOVER_CMD_POLYGON = 5;
        FLAG_TEXT_BOLD => CCLOVER_TEXT_BOLD = 1;
        FLAG_TEXT_END => CCLOVER_TEXT_END = 2;
        FLAG_TEXT_CLIP => CCLOVER_TEXT_CLIP = 4;
        FLAG_STATIC_CONTENT => CCLOVER_STATIC_CONTENT = 8;
    }

    structs {
        NativePoint => CcloverPoint {
            x: f32,
            y: f32,
        }

        NativeDamageRect => CcloverDamageRect {
            x1: f32,
            y1: f32,
            x2: f32,
            y2: f32,
        }

        NativeCommand => CcloverCommand {
            kind: u32,
            x: f32,
            y: f32,
            width: f32,
            height: f32,
            color: u32,
            radius: f32,
            stroke_width: f32,
            point_offset: usize,
            point_count: usize,
            text: const_u8_ptr,
            text_len: usize,
            text_size: u32,
            flags: u32,
        }

        SceneView => CcloverScene {
            width: u32,
            height: u32,
            static_revision: u64,
            full_redraw: u32,
            damage_rects: const_damage_rect_ptr,
            damage_count: usize,
            commands: const_command_ptr,
            command_count: usize,
            points: const_point_ptr,
            point_count: usize,
        }

        HostCallbacks => CcloverCallbacks {
            poll: poll_fn,
            scene: scene_fn,
        }
    }
}
