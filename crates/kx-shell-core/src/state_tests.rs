// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Unit tests for compositor state management.

use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::reexports::wayland_server::Display;
use smithay::utils::Transform;

use super::*;

#[test]
fn test_kynex_state_initialization() {
    let display: Display<KynexState> = Display::new().unwrap();
    let state = KynexState::new(
        &display.handle(),
        "wayland-test-0".into(),
        (1280, 720).into(),
    )
    .unwrap();

    assert_eq!(state.socket_name, "wayland-test-0");
    let mode = state.output.current_mode().unwrap();
    assert_eq!((mode.size, mode.refresh), ((1280, 720).into(), 60_000));
    assert!(matches!(state.output.current_scale(), Scale::Integer(1)));
    assert_eq!(state.output.current_transform(), Transform::Flipped180);

    state.update_output_size((1920, 1080).into());
    assert_eq!(
        state.output.current_mode().unwrap().size,
        (1920, 1080).into()
    );
}

#[test]
fn test_binding_matcher_emergency_exit() {
    let display = Display::<KynexState>::new().unwrap();
    let state = KynexState::new(
        &display.handle(),
        "wayland-test-exit".into(),
        (800, 600).into(),
    )
    .unwrap();

    assert_eq!(
        state.binding_matcher.matches(
            kx_input::KeyModifiers::CTRL | kx_input::KeyModifiers::SHIFT,
            kx_input::KeySym::ESCAPE,
        ),
        Some(&kx_input::CompositorAction::Quit)
    );
}

#[test]
fn test_map_unmap_output_and_migrate() {
    let display = Display::<KynexState>::new().unwrap();
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-test-multi".into(),
        (1280, 720).into(),
    )
    .unwrap();

    assert_eq!(state.space.outputs().count(), 1);

    let secondary = Output::new(
        "DRM-2".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Kynex".into(),
            model: "HDMI".into(),
        },
    );
    let sec_mode = Mode {
        size: (1920, 1080).into(),
        refresh: 60_000,
    };
    secondary.set_preferred(sec_mode);
    secondary.change_current_state(
        Some(sec_mode),
        Some(Transform::Normal),
        Some(Scale::Integer(1)),
        Some((1280, 0).into()),
    );

    state.map_output(&display.handle(), &secondary, (1280, 0));
    assert_eq!(state.space.outputs().count(), 2);

    state.unmap_output(&secondary);
    assert_eq!(state.space.outputs().count(), 1);

    state.migrate_windows_from_unplugged(
        (1280, 0).into(),
        (1920, 1080).into(),
        (0, 0).into(),
        (1280, 720).into(),
    );
    assert_eq!(state.space.outputs().count(), 1);
}
