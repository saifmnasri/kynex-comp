# kx-common

Shared leaf types (IDs, geometry newtypes, base error helpers, logging init) for KynexComp.

- May be depended on by every other `kx-*` crate.
- **Must not** depend on any other `kx-*` crate, `smithay`, `wayland-*`, `calloop`, or GPU crates.
