// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Unit tests for XDG activation protocol handling and resource limits.

use smithay::reexports::wayland_server::Display;

use super::*;
use crate::client::{ClientLimits, ClientState};

#[test]
fn test_activation_state_external_token_lifecycle() {
    let display = Display::<KynexState>::new().unwrap();
    let mut state = KynexState::new(
        &display.handle(),
        "wayland-activation-test".into(),
        (1280, 720).into(),
    )
    .unwrap();

    let (token, _data) = state.xdg_activation_state.create_external_token(None);
    let token_clone = token.clone();

    assert!(
        state
            .xdg_activation_state
            .data_for_token(&token_clone)
            .is_some()
    );
    assert_eq!(state.xdg_activation_state.tokens().count(), 1);

    state.purge_expired_tokens();
    assert_eq!(state.xdg_activation_state.tokens().count(), 1);

    assert!(state.xdg_activation_state.remove_token(&token_clone));
    assert_eq!(state.xdg_activation_state.tokens().count(), 0);
}

#[test]
fn test_activation_client_quota_bounding() {
    let limits = ClientLimits {
        max_activation_tokens: 2,
        ..ClientLimits::default()
    };
    let client_data = ClientState {
        limits,
        ..ClientState::default()
    };

    assert!(client_data.try_increment_activation_token());
    assert_eq!(
        client_data
            .activation_token_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert!(client_data.try_increment_activation_token());
    assert_eq!(
        client_data
            .activation_token_count
            .load(std::sync::atomic::Ordering::Relaxed),
        2
    );
    assert!(!client_data.try_increment_activation_token());

    client_data.decrement_activation_token();
    assert_eq!(
        client_data
            .activation_token_count
            .load(std::sync::atomic::Ordering::Relaxed),
        1
    );
    assert!(client_data.try_increment_activation_token());
}

#[test]
fn test_decrement_quota_none_client_id_is_noop() {
    let display = Display::<KynexState>::new().unwrap();
    let state = KynexState::new(
        &display.handle(),
        "wayland-activation-noop".into(),
        (1280, 720).into(),
    )
    .unwrap();

    state.decrement_token_client_quota(None);
}
