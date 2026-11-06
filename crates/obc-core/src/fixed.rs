//! 16.16 fixed point (`fixed` in `WOLFSRC/WL_DEF.H`).

/// One tile, or 1.0 (`GLOBAL1`, `TILEGLOBAL`).
pub const GLOBAL1: i32 = 1 << 16;
/// Shift from world coordinates to tile coordinates (`TILESHIFT`).
pub const TILESHIFT: u32 = 16;
/// Degrees in a circle for actor angles (`ANGLES`).
pub const ANGLES: i32 = 360;
/// Fine angles for rendering (`FINEANGLES`).
pub const FINEANGLES: i32 = 3600;
/// Minimum distance from a wall (`MINDIST`, `WOLFSRC/WL_DEF.H`).
pub const MINDIST: i32 = 0x5800;

/// `FixedByFrac` (`WOLFSRC/WL_DRAW.C`): multiplies `a` (two's complement 16.16)
/// by `b`, a **sign-magnitude** fraction (low word = magnitude, bit 31 = sign).
/// The 48-bit product is shifted right 16 and truncated to 32 bits, as the
/// original assembly does.
#[doc(alias = "FixedByFrac")]
pub fn fixed_by_frac(a: i32, b: i32) -> i32 {
    let frac = u64::from(b as u32 & 0xFFFF);
    let mut negative = (b as u32) & 0x8000_0000 != 0;
    let magnitude = if a < 0 {
        negative = !negative;
        a.wrapping_neg() as u32
    } else {
        a as u32
    };
    let r = ((u64::from(magnitude) * frac) >> 16) as u32 as i32;
    if negative { r.wrapping_neg() } else { r }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_by_frac_signs() {
        // 2.0 * 0.5 = 1.0
        assert_eq!(fixed_by_frac(2 * GLOBAL1, 0x8000), GLOBAL1);
        // 2.0 * -0.5 (sign-magnitude) = -1.0
        assert_eq!(fixed_by_frac(2 * GLOBAL1, 0x8000_8000u32 as i32), -GLOBAL1);
        // -2.0 * -0.5 = 1.0
        assert_eq!(fixed_by_frac(-2 * GLOBAL1, 0x8000_8000u32 as i32), GLOBAL1);
        // Truncation toward zero of the magnitude.
        assert_eq!(fixed_by_frac(-3, 0x8000), -1);
    }
}
