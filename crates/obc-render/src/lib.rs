//! Software renderer: 3D view, sprites, 2D drawing, palette effects (DESIGN §7.3).
//!
//! Everything draws into an indexed [`Framebuffer`]. The 3D view is a port of
//! Wolf3D's ray tracer (`WOLFSRC/WL_DR_A.ASM`, `WL_DRAW.C`, `WL_SCALE.C`)
//! parameterized by resolution: at the original view size it reproduces the
//! original integer pipeline (Classic), at larger sizes it renders the same
//! scene in high resolution and optionally widescreen (Enhanced).

pub mod draw2d;
mod framebuffer;
pub mod palette;
pub use obc_core::trig as tables;
pub mod view3d;

pub use framebuffer::Framebuffer;
