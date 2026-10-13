# kx-layout

Pure logic window and workspace layout engines, geometry constraints, focus stacks, and reachability validation for KynexComp.

## Purpose

Provides data structures, algorithms, and traits for managing the spatial layout and state of windows and workspaces without coupling to Wayland protocols, Smithay, or rendering engines.

## Public API (Phase 3)

- `WindowId`, `WindowState`, `WindowGeometry`, `SizeConstraints`, `WindowData`
- `WorkspaceId`, `Workspace`, `WorkspaceManager`
- `FocusStack`, `StackingOrder`, `FocusPolicy`, `ClickToFocusPolicy`
- `LayoutEngine`, `FloatingEngine`, `OutputGeometry`, `UsableArea`, `Reachability`
- `WindowManager`: Authoritative state machine maintaining consistency across all collections
- `DecorationLayout`, `DecorationHitTest`, `HitTestResult`: Pure geometry for titlebars and buttons

## Forbidden Dependencies

This crate is pure logic (Rule 02). It must NEVER depend on:
- `smithay`
- `wayland-server` or `wayland-client`
- `calloop`
- GPU, EGL, DRM, or rendering crates
- Blocking I/O or network crates
