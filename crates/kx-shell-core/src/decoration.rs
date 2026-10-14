// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Server-side window decoration management, hit-testing, and rendering.
//!
//! Handles `zxdg_decoration_manager_v1` negotiation, generates solid-color
//! render elements for titlebars and control buttons per ADR-0026, and provides
//! cursor and hit-testing helpers for interactive window management.

use smithay::backend::renderer::Color32F;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{Id, Kind};
use smithay::backend::renderer::utils::CommitCounter;
use smithay::desktop::Window;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use smithay::reexports::winit::window::CursorIcon;
use smithay::utils::{Logical, Physical, Point, Rectangle, Size};
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;

use kx_layout::{
    BORDER_MARGIN, BUTTON_SIZE, DecorationGeometry, DecorationHitTest, ResizeEdge, WindowGeometry,
    decoration_colors, hit_test_window,
};

use crate::state::KynexState;

/// Determines whether a desktop window is decorated by the compositor.
#[must_use]
pub fn window_has_ssd(window: &Window) -> bool {
    window
        .toplevel()
        .is_some_and(|t| t.current_state().decoration_mode == Some(Mode::ServerSide))
}

/// Maps a window resize edge to its matching bidirectional cursor icon.
#[must_use]
pub fn cursor_icon_for_edge(edge: ResizeEdge) -> CursorIcon {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => CursorIcon::NsResize,
        ResizeEdge::Left | ResizeEdge::Right => CursorIcon::EwResize,
        ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorIcon::NwseResize,
        ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorIcon::NeswResize,
    }
}

/// Maps a decoration hit test result to an optional winit cursor icon.
#[must_use]
pub fn cursor_icon_for_hit_test(hit: &DecorationHitTest) -> Option<CursorIcon> {
    match hit {
        DecorationHitTest::ResizeBorder(edge) => Some(cursor_icon_for_edge(*edge)),
        DecorationHitTest::CloseButton
        | DecorationHitTest::MaximizeButton
        | DecorationHitTest::MinimizeButton => Some(CursorIcon::Pointer),
        DecorationHitTest::Titlebar => Some(CursorIcon::Default),
        DecorationHitTest::Content | DecorationHitTest::Outside => None,
    }
}

/// Generates solid-color render elements for an SSD-decorated window.
///
/// Returns button elements followed by the titlebar element so that in Smithay's
/// `.rev()` render loop, the titlebar is drawn first and the buttons on top.
fn button_element(pos: (i32, i32), color_arr: [f32; 4]) -> SolidColorRenderElement {
    let color = Color32F::new(color_arr[0], color_arr[1], color_arr[2], color_arr[3]);
    let rect = Rectangle::new(
        Point::<i32, Physical>::from(pos),
        Size::<i32, Physical>::from((BUTTON_SIZE, BUTTON_SIZE)),
    );
    SolidColorRenderElement::new(
        Id::new(),
        rect,
        CommitCounter::default(),
        color,
        Kind::Unspecified,
    )
}

/// Generates solid-color render elements for an SSD-decorated window.
///
/// Returns button elements followed by the titlebar element so that in Smithay's
/// `.rev()` render loop, the titlebar is drawn first and the buttons on top.
#[must_use]
pub fn render_elements_for_window(
    geo: WindowGeometry,
    is_active: bool,
) -> Vec<SolidColorRenderElement> {
    let dec = DecorationGeometry::for_window(geo);
    let titlebar_arr = if is_active {
        decoration_colors::TITLEBAR_ACTIVE
    } else {
        decoration_colors::TITLEBAR_INACTIVE
    };
    let titlebar_color = Color32F::new(
        titlebar_arr[0],
        titlebar_arr[1],
        titlebar_arr[2],
        titlebar_arr[3],
    );

    let titlebar_rect = Rectangle::new(
        Point::<i32, Physical>::from((dec.titlebar.x, dec.titlebar.y)),
        Size::<i32, Physical>::from((dec.titlebar.width, dec.titlebar.height)),
    );
    let titlebar_elem = SolidColorRenderElement::new(
        Id::new(),
        titlebar_rect,
        CommitCounter::default(),
        titlebar_color,
        Kind::Unspecified,
    );

    let close_elem = button_element(
        (dec.close_button.x, dec.close_button.y),
        decoration_colors::CLOSE_BUTTON,
    );
    let max_elem = button_element(
        (dec.maximize_button.x, dec.maximize_button.y),
        decoration_colors::MAXIMIZE_BUTTON,
    );
    let min_elem = button_element(
        (dec.minimize_button.x, dec.minimize_button.y),
        decoration_colors::MINIMIZE_BUTTON,
    );

    // In Smithay's damage_tracker::render_output, elements are drawn in reverse.
    // Placing buttons before titlebar ensures titlebar is rendered first and buttons on top.
    vec![close_elem, max_elem, min_elem, titlebar_elem]
}

/// Collects solid-color render elements for all SSD windows on the desktop.
#[must_use]
pub fn collect_ssd_render_elements(state: &KynexState) -> Vec<SolidColorRenderElement> {
    let mut elements = Vec::new();
    let focused_wid = state.window_manager.focus().focused();

    // Iterate top-to-bottom so uppermost windows are rendered on top in Smithay's .rev() loop.
    for window in state.space.elements().rev() {
        if !window_has_ssd(window) {
            continue;
        }
        let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) else {
            continue;
        };
        let Some(&wid) = state.surface_to_window_id.get(&surface) else {
            continue;
        };
        let Some(win) = state.window_manager.window(wid) else {
            continue;
        };
        if win.state == kx_layout::WindowState::Fullscreen {
            continue;
        }
        let is_active = Some(wid) == focused_wid;
        elements.extend(render_elements_for_window(win.geometry, is_active));
    }

    elements
}

/// Finds the uppermost window and hit test region under the given pointer position.
#[must_use]
pub fn find_window_hit_test(
    state: &KynexState,
    point: Point<f64, Logical>,
) -> Option<(kx_layout::WindowId, DecorationHitTest)> {
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer coordinates are bounded to display dimensions and fit within i32"
    )]
    let px = point.x.round() as i32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "pointer coordinates are bounded to display dimensions and fit within i32"
    )]
    let py = point.y.round() as i32;

    for window in state.space.elements().rev() {
        let Some(surface) = window.toplevel().map(|t| t.wl_surface().clone()) else {
            continue;
        };
        let Some(&wid) = state.surface_to_window_id.get(&surface) else {
            continue;
        };
        let Some(win) = state.window_manager.window(wid) else {
            continue;
        };
        if win.state == kx_layout::WindowState::Fullscreen {
            if px >= win.geometry.x
                && px < win.geometry.x.saturating_add(win.geometry.width)
                && py >= win.geometry.y
                && py < win.geometry.y.saturating_add(win.geometry.height)
            {
                return Some((wid, DecorationHitTest::Content));
            }
            continue;
        }
        let has_ssd = window_has_ssd(window);
        let hit = hit_test_window(px, py, win.geometry, has_ssd, BORDER_MARGIN);
        if hit != DecorationHitTest::Outside {
            return Some((wid, hit));
        }
    }

    None
}

impl XdgDecorationHandler for KynexState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, mode: Mode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(mode);
        });
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(Mode::ServerSide);
        });
        toplevel.send_configure();
    }
}

/// Converts internal layout [`ResizeEdge`] to Wayland protocol `xdg_toplevel::ResizeEdge`.
#[must_use]
pub fn to_xdg_resize_edge(
    edge: ResizeEdge,
) -> smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge as XdgEdge;
    match edge {
        ResizeEdge::Top => XdgEdge::Top,
        ResizeEdge::Bottom => XdgEdge::Bottom,
        ResizeEdge::Left => XdgEdge::Left,
        ResizeEdge::Right => XdgEdge::Right,
        ResizeEdge::TopLeft => XdgEdge::TopLeft,
        ResizeEdge::TopRight => XdgEdge::TopRight,
        ResizeEdge::BottomLeft => XdgEdge::BottomLeft,
        ResizeEdge::BottomRight => XdgEdge::BottomRight,
    }
}

/// Handles a mouse button press on window decorations.
///
/// Returns `true` if the event fell within a titlebar, border, or button and was handled.
pub fn handle_decoration_click(
    state: &mut KynexState,
    pointer_location: Point<f64, Logical>,
    button: u32,
    serial: smithay::utils::Serial,
) -> bool {
    if button != crate::grab::BTN_LEFT {
        return false;
    }
    let Some((wid, hit)) = find_window_hit_test(state, pointer_location) else {
        return false;
    };

    match hit {
        DecorationHitTest::ResizeBorder(edges) => {
            crate::action::activate_and_focus_window(state, wid);
            crate::grab::start_compositor_resize_grab(
                state,
                wid,
                pointer_location,
                to_xdg_resize_edge(edges),
                button,
                serial,
            );
            true
        }
        DecorationHitTest::Titlebar => {
            crate::action::activate_and_focus_window(state, wid);
            crate::grab::start_compositor_move_grab(state, wid, pointer_location, button, serial);
            true
        }
        DecorationHitTest::CloseButton
        | DecorationHitTest::MaximizeButton
        | DecorationHitTest::MinimizeButton => {
            crate::action::activate_and_focus_window(state, wid);
            let action = match hit {
                DecorationHitTest::CloseButton => kx_input::CompositorAction::CloseWindow,
                DecorationHitTest::MaximizeButton => kx_input::CompositorAction::ToggleMaximize,
                _ => kx_input::CompositorAction::MinimizeWindow,
            };
            crate::action::execute_compositor_action(state, &action);
            true
        }
        DecorationHitTest::Content | DecorationHitTest::Outside => false,
    }
}
