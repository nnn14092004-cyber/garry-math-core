//! Logical execution clock and bounded computational step metering for halting guarantees.

use crate::EngineError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LogicalClock {
    epoch: u32,
    tick: u64,
}

impl Default for LogicalClock {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicalClock {
    pub const fn new() -> Self {
        Self { epoch: 0, tick: 0 }
    }

    pub const fn from_raw(epoch: u32, tick: u64) -> Self {
        Self { epoch, tick }
    }

    pub fn advance_tick(&mut self) -> u64 {
        self.tick = self.tick.saturating_add(1);
        self.tick
    }

    pub fn advance_epoch(&mut self) -> u32 {
        self.epoch = self.epoch.saturating_add(1);
        self.epoch
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn epoch(&self) -> u32 {
        self.epoch
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeterConfig {
    pub max_steps: u32,
    pub max_units: u64,
    pub step_cost: u64,
    pub transition_cost: u64,
    pub storage_read_byte_cost: u64,
    pub storage_write_byte_cost: u64,
    pub prng_cost: u64,
}

impl MeterConfig {
    pub const fn standard(max_steps: u32) -> Self {
        Self {
            max_steps,
            max_units: (max_steps as u64).saturating_mul(1_000),
            step_cost: 100,
            transition_cost: 20,
            storage_read_byte_cost: 1,
            storage_write_byte_cost: 4,
            prng_cost: 5,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepMeter {
    config: MeterConfig,
    consumed_steps: u32,
    consumed_units: u64,
}

impl StepMeter {
    pub const fn new(config: MeterConfig) -> Self {
        Self {
            config,
            consumed_steps: 0,
            consumed_units: 0,
        }
    }

    pub const fn from_raw(config: MeterConfig, consumed_steps: u32, consumed_units: u64) -> Self {
        Self {
            config,
            consumed_steps,
            consumed_units,
        }
    }

    pub const fn config(&self) -> &MeterConfig {
        &self.config
    }

    pub fn charge_step(&mut self) -> Result<(), EngineError> {
        let next_step = self.consumed_steps.saturating_add(1);
        match next_step > self.config.max_steps {
            true => Err(EngineError::BudgetExceeded {
                limit: self.config.max_steps,
                current: next_step,
            }),
            false => {
                self.charge_units(self.config.step_cost)?;
                self.consumed_steps = next_step;
                Ok(())
            }
        }
    }

    pub fn charge_units(&mut self, units: u64) -> Result<(), EngineError> {
        let next_units = self.consumed_units.saturating_add(units);
        match next_units > self.config.max_units {
            true => Err(EngineError::QuotaExceeded {
                limit: self.config.max_units,
                current: next_units,
            }),
            false => {
                self.consumed_units = next_units;
                Ok(())
            }
        }
    }

    pub fn charge_transition(&mut self) -> Result<(), EngineError> {
        self.charge_units(self.config.transition_cost)
    }

    pub fn charge_storage_read(&mut self, bytes_len: usize) -> Result<(), EngineError> {
        let cost = (bytes_len as u64).saturating_mul(self.config.storage_read_byte_cost);
        self.charge_units(cost)
    }

    pub fn charge_storage_write(&mut self, bytes_len: usize) -> Result<(), EngineError> {
        let cost = (bytes_len as u64).saturating_mul(self.config.storage_write_byte_cost);
        self.charge_units(cost)
    }

    pub fn charge_prng(&mut self) -> Result<(), EngineError> {
        self.charge_units(self.config.prng_cost)
    }

    pub const fn consumed_steps(&self) -> u32 {
        self.consumed_steps
    }

    pub const fn consumed_units(&self) -> u64 {
        self.consumed_units
    }

    pub const fn remaining_steps(&self) -> u32 {
        self.config.max_steps.saturating_sub(self.consumed_steps)
    }

    pub const fn remaining_units(&self) -> u64 {
        self.config.max_units.saturating_sub(self.consumed_units)
    }
}