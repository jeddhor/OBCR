//! Trigonometric tables (`BuildTables`, `WOLFSRC/WL_MAIN.C`).
//!
//! The original computes these with Borland floating point at startup. We
//! repeat the same arithmetic (f32 where the original used `float`). Exactness
//! against the original's tables is to be verified with an oracle memory dump
//! (MILESTONES M4.R2); until then this is LIKELY identical.

/// Fine angles in a circle (`FINEANGLES`).
pub const FINEANGLES: i32 = 3600;
/// Coarse angles (`ANGLES`).
pub const ANGLES: usize = 360;
const ANGLEQUAD: usize = ANGLES / 4;
const TILEGLOBAL: f64 = 65536.0;
/// `PI` as defined in `WOLFSRC/WL_DEF.H` (not `std::f64::consts::PI`).
const PI: f64 = 3.141592657;

/// `radtoint`: a `const float` in the original.
pub fn radtoint() -> f64 {
    f64::from((FINEANGLES as f64 / 2.0 / PI) as f32)
}

/// Shared trig tables.
#[derive(Debug, Clone)]
pub struct Tables {
    /// `finetangent[FINEANGLES/4]`, 16.16 fixed.
    pub finetangent: Vec<i32>,
    /// `sintable[ANGLES + ANGLES/4 + 1]`, **sign-magnitude** 16.16 (bit 31 = sign).
    /// `costable` is `sintable[ANGLES/4..]`.
    pub sintable: Vec<i32>,
}

impl Tables {
    pub fn build() -> Self {
        let r = radtoint();
        let mut finetangent = vec![0i32; (FINEANGLES / 4) as usize];
        for i in 0..(FINEANGLES / 8) as usize {
            let tang = ((i as f64 + 0.5) / r).tan();
            finetangent[i] = (tang * TILEGLOBAL) as i32;
            finetangent[(FINEANGLES / 4) as usize - 1 - i] = (1.0 / tang * TILEGLOBAL) as i32;
        }
        let mut sintable = vec![0i32; ANGLES + ANGLEQUAD + 1];
        let mut angle: f32 = 0.0;
        let anglestep = (PI / 2.0 / ANGLEQUAD as f64) as f32;
        for i in 0..=ANGLEQUAD {
            let value = (65536.0 * f64::from(angle).sin()) as i32;
            sintable[i] = value;
            sintable[i + ANGLES] = value;
            sintable[ANGLES / 2 - i] = value;
            sintable[ANGLES - i] = value | i32::MIN;
            sintable[ANGLES / 2 + i] = value | i32::MIN;
            angle += anglestep;
        }
        Self { finetangent, sintable }
    }

    /// `sintable[angle]` (sign-magnitude).
    pub fn sin(&self, angle: usize) -> i32 {
        self.sintable[angle]
    }

    /// `costable[angle]` = `sintable[angle + ANGLES/4]` (sign-magnitude).
    pub fn cos(&self, angle: usize) -> i32 {
        self.sintable[angle + ANGLEQUAD]
    }
}

/// Converts a sign-magnitude table value to two's complement.
pub fn sm_to_i32(v: i32) -> i32 {
    if v < 0 { -(v & i32::MAX) } else { v }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_have_expected_shape() {
        let t = Tables::build();
        assert_eq!(t.sin(0), 0);
        // The accumulated `float` angle lands just past pi/2, so the original's
        // table likely holds 65535 here; confirm against an oracle dump (M4.R2).
        assert!((65535..=65536).contains(&t.sin(90)));
        assert_eq!(sm_to_i32(t.sin(270)), -t.sin(90));
        assert_eq!(sm_to_i32(t.cos(180)), -t.sin(90));
        // tan(0.05 deg) * 65536 ≈ 57.
        assert!((50..65).contains(&t.finetangent[0]));
        assert!(t.finetangent[899] > 1_000_000);
    }
}
