// Copyright (c) 2026 Kynex. All rights reserved.
// Proprietary and confidential. See LICENSE.

//! Pure-logic multi-output layout engine and KMS CRTC allocation.
//!
//! Provides deterministic matching of connected DRM display connectors to
//! available CRTCs, automatic multi-monitor horizontal layout placement,
//! and window migration calculations when outputs are dynamically unplugged.

use std::collections::HashSet;
use std::hash::Hash;

use smithay::utils::{Logical, Point, Size};

/// Description of a connected display connector candidate for modesetting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorCandidate<C> {
    /// Connector identifier or handle.
    pub handle: C,
    /// Physical or human-readable interface name (e.g. "eDP-1", "HDMI-A-1").
    pub name: String,
    /// Display resolution in physical/mode pixels.
    pub size: Size<i32, Logical>,
    /// Compatible CRTC indices or handles that can drive this connector.
    ///
    /// If empty, any available CRTC is considered compatible.
    pub possible_crtcs: Vec<u32>,
}

/// An allocated display output ready for modeset configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputAssignment<C, R> {
    /// Assigned connector handle.
    pub connector: C,
    /// Assigned connector name.
    pub name: String,
    /// Assigned CRTC handle or identifier.
    pub crtc: R,
    /// Display logical resolution.
    pub size: Size<i32, Logical>,
    /// Logical coordinate position in the compositor's global space.
    pub position: Point<i32, Logical>,
}

/// Computes a deterministic, collision-free assignment of connected connectors to available CRTCs,
/// and places assigned displays side-by-side in global coordinate space.
///
/// # Arguments
///
/// * `candidates` - List of detected connected display connectors.
/// * `available_crtcs` - List of available CRTC handles on the DRM device.
///
/// Returns a list of [`OutputAssignment`] pairings. If there are fewer CRTCs than connected
/// displays, unallocated connectors are safely skipped without error.
#[must_use]
pub fn allocate_outputs<C: Copy + Eq, R: Copy + Eq + Hash>(
    candidates: &[ConnectorCandidate<C>],
    available_crtcs: &[R],
) -> Vec<OutputAssignment<C, R>> {
    let mut assignments = Vec::with_capacity(candidates.len().min(available_crtcs.len()));
    let mut used_crtcs = HashSet::with_capacity(available_crtcs.len());
    let mut current_x: i32 = 0;

    for candidate in candidates {
        // Find the first available CRTC compatible with this connector
        let matched_crtc = available_crtcs
            .iter()
            .enumerate()
            .find(|(idx, crtc)| {
                if used_crtcs.contains(*crtc) {
                    return false;
                }
                if candidate.possible_crtcs.is_empty() {
                    return true;
                }
                #[expect(
                    clippy::cast_possible_truncation,
                    reason = "CRTC count on DRM device is small (typically <= 8)"
                )]
                candidate.possible_crtcs.contains(&(*idx as u32))
            })
            .map(|(_, crtc)| *crtc);

        if let Some(crtc) = matched_crtc {
            used_crtcs.insert(crtc);
            let position = Point::from((current_x, 0));
            current_x = current_x.saturating_add(candidate.size.w);

            assignments.push(OutputAssignment {
                connector: candidate.handle,
                name: candidate.name.clone(),
                crtc,
                size: candidate.size,
                position,
            });
        }
    }

    assignments
}

/// Computes target coordinates for a window located on an unplugged display,
/// ensuring it remains visible on an active surviving display.
///
/// If no active outputs remain, the window's position is returned unchanged.
#[must_use]
pub fn migrate_window_position(
    window_loc: Point<i32, Logical>,
    unplugged_origin: Point<i32, Logical>,
    unplugged_size: Size<i32, Logical>,
    primary_origin: Point<i32, Logical>,
    primary_size: Size<i32, Logical>,
) -> Point<i32, Logical> {
    // Check if the window was within the unplugged output's bounding box
    let in_unplugged_x = window_loc.x >= unplugged_origin.x
        && window_loc.x < unplugged_origin.x.saturating_add(unplugged_size.w);
    let in_unplugged_y = window_loc.y >= unplugged_origin.y
        && window_loc.y < unplugged_origin.y.saturating_add(unplugged_size.h);

    if in_unplugged_x && in_unplugged_y {
        // Calculate relative offset within the unplugged screen
        let rel_x = window_loc.x.saturating_sub(unplugged_origin.x);
        let rel_y = window_loc.y.saturating_sub(unplugged_origin.y);

        // Clamp inside primary screen boundaries
        let max_x = primary_origin
            .x
            .saturating_add(primary_size.w.saturating_sub(100));
        let max_y = primary_origin
            .y
            .saturating_add(primary_size.h.saturating_sub(100));

        let new_x = primary_origin
            .x
            .saturating_add(rel_x)
            .min(max_x.max(primary_origin.x));
        let new_y = primary_origin
            .y
            .saturating_add(rel_y)
            .min(max_y.max(primary_origin.y));

        Point::from((new_x, new_y))
    } else {
        window_loc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate_single_output() {
        let candidates = [ConnectorCandidate {
            handle: 10u32,
            name: "eDP-1".to_string(),
            size: Size::from((1920, 1080)),
            possible_crtcs: vec![],
        }];
        let crtcs = [100u32, 101u32];

        let assignments = allocate_outputs(&candidates, &crtcs);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].connector, 10);
        assert_eq!(assignments[0].crtc, 100);
        assert_eq!(assignments[0].position, Point::from((0, 0)));
    }

    #[test]
    fn test_allocate_multi_output_horizontal_strip() {
        let candidates = [
            ConnectorCandidate {
                handle: 10u32,
                name: "eDP-1".to_string(),
                size: Size::from((1920, 1080)),
                possible_crtcs: vec![],
            },
            ConnectorCandidate {
                handle: 11u32,
                name: "HDMI-A-1".to_string(),
                size: Size::from((2560, 1440)),
                possible_crtcs: vec![],
            },
        ];
        let crtcs = [100u32, 101u32];

        let assignments = allocate_outputs(&candidates, &crtcs);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].connector, 10);
        assert_eq!(assignments[0].crtc, 100);
        assert_eq!(assignments[0].position, Point::from((0, 0)));

        assert_eq!(assignments[1].connector, 11);
        assert_eq!(assignments[1].crtc, 101);
        assert_eq!(assignments[1].position, Point::from((1920, 0)));
    }

    #[test]
    fn test_allocate_more_connectors_than_crtcs() {
        let candidates = [
            ConnectorCandidate {
                handle: 10u32,
                name: "eDP-1".to_string(),
                size: Size::from((1920, 1080)),
                possible_crtcs: vec![],
            },
            ConnectorCandidate {
                handle: 11u32,
                name: "HDMI-A-1".to_string(),
                size: Size::from((1920, 1080)),
                possible_crtcs: vec![],
            },
        ];
        let crtcs = [100u32]; // Only 1 CRTC available

        let assignments = allocate_outputs(&candidates, &crtcs);
        assert_eq!(assignments.len(), 1);
        assert_eq!(assignments[0].connector, 10);
        assert_eq!(assignments[0].crtc, 100);
    }

    #[test]
    fn test_allocate_constrained_crtcs() {
        let candidates = [
            ConnectorCandidate {
                handle: 10u32,
                name: "eDP-1".to_string(),
                size: Size::from((1920, 1080)),
                possible_crtcs: vec![1], // Only CRTC index 1
            },
            ConnectorCandidate {
                handle: 11u32,
                name: "HDMI-A-1".to_string(),
                size: Size::from((1920, 1080)),
                possible_crtcs: vec![0], // Only CRTC index 0
            },
        ];
        let crtcs = [100u32, 101u32];

        let assignments = allocate_outputs(&candidates, &crtcs);
        assert_eq!(assignments.len(), 2);
        assert_eq!(assignments[0].connector, 10);
        assert_eq!(assignments[0].crtc, 101); // matched index 1

        assert_eq!(assignments[1].connector, 11);
        assert_eq!(assignments[1].crtc, 100); // matched index 0
    }

    #[test]
    fn test_migrate_window_position_on_unplug() {
        let unplugged_origin = Point::from((1920, 0));
        let unplugged_size = Size::from((1920, 1080));
        let primary_origin = Point::from((0, 0));
        let primary_size = Size::from((1920, 1080));

        // Window on secondary unplugged screen at (2000, 100)
        let window_loc = Point::from((2000, 100));
        let migrated = migrate_window_position(
            window_loc,
            unplugged_origin,
            unplugged_size,
            primary_origin,
            primary_size,
        );

        // Relative offset was 80, 100 -> placed on primary screen at (80, 100)
        assert_eq!(migrated, Point::from((80, 100)));

        // Window already on primary screen at (100, 200) stays untouched
        let primary_window = Point::from((100, 200));
        let unchanged = migrate_window_position(
            primary_window,
            unplugged_origin,
            unplugged_size,
            primary_origin,
            primary_size,
        );
        assert_eq!(unchanged, Point::from((100, 200)));
    }
}
