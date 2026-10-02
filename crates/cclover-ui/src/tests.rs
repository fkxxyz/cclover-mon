use cclover_core::model::{
    Collection, DiskMetadata, DiskSnapshot, MonitorState, NetworkSnapshot, TemperatureSnapshot,
};
use cclover_presentation::{Dashboard, FAN_SECTION};

use super::*;
use crate::cards::{disk_card, network_card};

#[test]
fn dashboard_tree_drives_dynamic_height() {
    let empty = MonitorState::default();
    let empty_ui = DashboardUi::new(Dashboard::new(&empty));
    assert_eq!(empty_ui.left.len(), 4);
    assert!(matches!(
        empty_ui.left.last(),
        Some(Block::Section(section)) if section.text.as_ref() == FAN_SECTION
    ));
    assert_eq!(empty_ui.right.len(), 3);

    let mut populated = MonitorState::default();
    populated.snapshot.temperatures = Collection::available(vec![TemperatureSnapshot {
        id: cclover_core::model::TemperatureId::from_opaque_key("cpu-temperature"),
        name: "CPU".to_owned(),
        celsius: 50.0,
    }]);
    let populated_ui = DashboardUi::new(Dashboard::new(&populated));
    assert_eq!(populated_ui.left.len(), 5);
    assert!(populated_ui.height() > empty_ui.height());
}

#[test]
fn io_top_regions_are_fixed_when_observable_and_omitted_when_unavailable() {
    let mut state = MonitorState::default();
    state.snapshot.disks = Collection::available(vec![DiskSnapshot {
        id: cclover_core::model::DiskId::from_opaque_key("disk-a"),
        metadata: DiskMetadata {
            system_label: "disk-a".into(),
            associated_labels: Vec::new(),
        },
        bytes_per_sec: 0.0,
    }]);
    state.snapshot.networks = Collection::available(vec![NetworkSnapshot {
        id: cclover_core::model::NetworkId::from_opaque_key("network-a"),
        name: "network-a".into(),
        down_bytes_per_sec: 0.0,
        up_bytes_per_sec: 0.0,
    }]);

    let dashboard = Dashboard::new(&state);
    let unavailable_disk_height = disk_card(dashboard.disk(0).unwrap(), 1).height();
    let unavailable_network_height = network_card(dashboard.network(0).unwrap(), 1).height();

    state.snapshot.process_disk_io = Collection::available(Vec::new());
    state.snapshot.process_network_io = Collection::available(Vec::new());
    let dashboard = Dashboard::new(&state);
    let available_disk_height = disk_card(dashboard.disk(0).unwrap(), 1).height();
    let available_network_height = network_card(dashboard.network(0).unwrap(), 1).height();

    assert_eq!(
        available_disk_height - unavailable_disk_height,
        IO_PROCESS_GEOMETRY.height() + DISK_CARD_GEOMETRY.spacing
    );
    assert_eq!(
        available_network_height - unavailable_network_height,
        IO_PROCESS_GEOMETRY.height() + NETWORK_CARD_GEOMETRY.spacing
    );
}

#[test]
fn tree_owns_visual_semantics_not_renderer_types() {
    let state = MonitorState::default();
    let ui = DashboardUi::new(Dashboard::new(&state));
    let Block::Card(memory) = &ui.left[0] else {
        panic!("memory must be a card");
    };
    let Element::Row(header) = &memory.content.children[0] else {
        panic!("memory card must start with a row");
    };
    assert_eq!(header.cells[0].text, "MEMORY");
    assert_eq!(header.cells[0].tone, Tone::Muted);
    assert_eq!(header.cells[0].width, CellWidth::Fill);
    assert_eq!(Tone::Accent.rgba().b, 0xff);
}
