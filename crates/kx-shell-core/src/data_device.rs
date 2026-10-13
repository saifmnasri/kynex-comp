// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Data device protocol delegates (`wl_data_device_manager`).

use std::os::unix::io::OwnedFd;

use smithay::delegate_data_device;
use smithay::input::Seat;
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};

use crate::state::KynexState;

impl SelectionHandler for KynexState {
    type SelectionUserData = ();
}

impl DataDeviceHandler for KynexState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for KynexState {}

impl ServerDndGrabHandler for KynexState {
    fn send(&mut self, _mime_type: String, _fd: OwnedFd, _seat: Seat<Self>) {}
}

delegate_data_device!(KynexState);
