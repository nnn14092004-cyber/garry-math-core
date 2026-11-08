//! Deterministic Xorshift64* pseudo-random number generator for polynomial identity testing.

use crate::metering::StepMeter;
use crate::EngineError;

pub const PRNG_FALLBACK_SEED: u64 = 0x9E37_79B9_7F4A_7C15;
pub const XORSHIFT_STAR_MULTIPLIER: u64 = 0x2545_F491_4F6C_DD1D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xorshift64Star {
    state: u64,
}

impl Xorshift64Star {
    pub const fn new(seed: u64) -> Self {
        let state = match seed {
            0 => PRNG_FALLBACK_SEED,
            s => s,
        };
        Self { state }
    }

    pub const fn state(&self) -> u64 {
        self.state
    }

    pub const fn from_raw(state: u64) -> Self {
        Self::new(state)
    }

    pub fn next_u64(&mut self, meter: &mut StepMeter) -> Result<u64, EngineError> {
        meter.charge_prng()?;
        Ok(self.step())
    }

    pub fn next_in_range(
        &mut self,
        meter: &mut StepMeter,
        min: u64,
        max: u64,
    ) -> Result<u64, EngineError> {
        match min >= max {
            true => Ok(min),
            false => {
                let range = max - min + 1;
                let rand = self.next_u64(meter)?;
                Ok(min + (rand % range))
            }
        }
    }

    pub fn step(&mut self) -> u64 {
        let mut x = match self.state {
            0 => PRNG_FALLBACK_SEED,
            s => s,
        };
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = match x {
            0 => PRNG_FALLBACK_SEED,
            val => val,
        };
        self.state.wrapping_mul(XORSHIFT_STAR_MULTIPLIER)
    }
}