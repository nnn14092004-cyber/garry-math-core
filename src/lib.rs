//! Core deterministic state machine, bounded interval SMT constraint solver,
//! and 4-round mathematical refutation kernel.

pub mod crc;
pub mod ephemeral;
pub mod hypergraph;
pub mod metering;
pub mod plan;
pub mod planner;
pub mod prng;
pub mod quantum;
pub mod solver;
pub mod spatial;
pub mod symbolic;

use std::fmt;
use std::marker::PhantomData;

pub use crc::{
    calculate_crc32, compute_crc32, crc32, make_crc_table, CRC32_POLYNOMIAL, CRC_TABLE,
};
pub use hypergraph::{
    AssociativeHyperEdge, AxiomHyperGraphL2, HyperGraphAxiomNode, DECAY_EVICTION_THRESHOLD,
    HYPERGRAPH_L2_CAPACITY, INITIAL_VITALITY_SCORE, L1_HOT_SLOTS, MAX_HYPEREDGES,
};
pub use metering::{LogicalClock, MeterConfig, StepMeter};
pub use plan::{NodeStatus, PlanGraph, PlanNode, MAX_DEPENDENCIES, MAX_PLAN_NODES};
pub use prng::{Xorshift64Star, PRNG_FALLBACK_SEED, XORSHIFT_STAR_MULTIPLIER};

pub const AUDIT_LOG_CAPACITY: usize = 64;
pub const MAX_KEY_SIZE: usize = 32;
pub const MAX_VALUE_SIZE: usize = 256;
pub const MAX_STORAGE_SLOTS: usize = 16;
pub const MAX_REGISTERED_ACTIONS: usize = 16;

pub const CHECKPOINT_MAGIC: &[u8; 8] = b"GARRYMTH";
pub const CHECKPOINT_VERSION: u16 = 3;
pub const MAX_EVENT_TAG_SIZE: usize = 32;
pub const CHECKPOINT_SIZE: usize = 7865;
pub const DEFAULT_PRNG_SEED: u64 = 0x517C_C1B7_2722_0A95;

pub const MAX_SMT_VARS: usize = 16;
pub const MAX_SMT_CONSTRAINTS: usize = 32;
pub const MAX_SMT_PROPAGATION_STEPS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtRelationOp {
    GreaterThan,
    GreaterOrEqual,
    LessThan,
    LessOrEqual,
    Equal,
    NotEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmtDomainInterval {
    pub var_hash: u32,
    pub min_val: i128,
    pub max_val: i128,
    pub excludes_zero: bool,
}

impl SmtDomainInterval {
    pub const fn unbounded(var_hash: u32) -> Self {
        Self {
            var_hash,
            min_val: 0,
            max_val: i128::MAX / 2,
            excludes_zero: false,
        }
    }

    pub const fn is_contradictory(&self) -> bool {
        let inverted = self.min_val > self.max_val;
        let trapped_at_excluded_zero =
            self.min_val == 0 && self.max_val == 0 && self.excludes_zero;
        inverted || trapped_at_excluded_zero
    }

    pub const fn can_be_zero(&self) -> bool {
        let spans_zero = self.min_val <= 0 && self.max_val >= 0;
        spans_zero && !self.excludes_zero
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmtConstraint {
    pub lhs_var_hash: u32,
    pub op: SmtRelationOp,
    pub rhs_constant: i128,
    pub rhs_var_hash: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmtSatResult {
    Satisfiable { witness_value: i128 },
    Unsatisfiable,
}

#[derive(Debug, Clone, Copy)]
pub struct DeterministicSmtProver {
    pub domains: [Option<SmtDomainInterval>; MAX_SMT_VARS],
    pub domain_count: usize,
    pub constraints: [Option<SmtConstraint>; MAX_SMT_CONSTRAINTS],
    pub constraint_count: usize,
}

impl Default for DeterministicSmtProver {
    fn default() -> Self {
        Self::new()
    }
}

impl DeterministicSmtProver {
    pub const fn new() -> Self {
        Self {
            domains: [None; MAX_SMT_VARS],
            domain_count: 0,
            constraints: [None; MAX_SMT_CONSTRAINTS],
            constraint_count: 0,
        }
    }

    pub fn ensure_variable(&mut self, var_hash: u32) -> Option<usize> {
        let mut idx = 0usize;
        while idx < self.domain_count {
            match self.domains[idx] {
                Some(dom) => match dom.var_hash == var_hash {
                    true => return Some(idx),
                    false => {}
                },
                None => {}
            }
            idx += 1;
        }

        match self.domain_count < MAX_SMT_VARS {
            true => {
                let slot = self.domain_count;
                self.domains[slot] = Some(SmtDomainInterval::unbounded(var_hash));
                self.domain_count += 1;
                Some(slot)
            }
            false => None,
        }
    }

    pub fn add_precondition(&mut self, constraint: SmtConstraint) -> bool {
        let _ = self.ensure_variable(constraint.lhs_var_hash);
        match constraint.rhs_var_hash {
            Some(rhs_h) => {
                let _ = self.ensure_variable(rhs_h);
            }
            None => {}
        }

        match self.constraint_count < MAX_SMT_CONSTRAINTS {
            true => {
                self.constraints[self.constraint_count] = Some(constraint);
                self.constraint_count += 1;
                true
            }
            false => false,
        }
    }

    pub fn propagate_bounds(&mut self) -> bool {
        let mut step = 0usize;
        while step < MAX_SMT_PROPAGATION_STEPS {
            step += 1;
            let mut changed = false;
            let mut c_idx = 0usize;

            while c_idx < self.constraint_count {
                match self.constraints[c_idx] {
                    Some(c) => {
                        let rhs_bound = match c.rhs_var_hash {
                            Some(rhs_h) => self.lookup_domain_min(rhs_h).unwrap_or(c.rhs_constant),
                            None => c.rhs_constant,
                        };

                        match self.ensure_variable(c.lhs_var_hash) {
                            Some(slot) => match self.domains[slot] {
                                Some(mut dom) => {
                                    let prev = dom;
                                    match c.op {
                                        SmtRelationOp::GreaterThan => {
                                            let target_min = rhs_bound.saturating_add(1);
                                            dom.min_val = dom.min_val.max(target_min);
                                            match rhs_bound >= 0 {
                                                true => dom.excludes_zero = true,
                                                false => {}
                                            }
                                        }
                                        SmtRelationOp::GreaterOrEqual => {
                                            dom.min_val = dom.min_val.max(rhs_bound);
                                            match rhs_bound > 0 {
                                                true => dom.excludes_zero = true,
                                                false => {}
                                            }
                                        }
                                        SmtRelationOp::LessThan => {
                                            let target_max = rhs_bound.saturating_sub(1);
                                            dom.max_val = dom.max_val.min(target_max);
                                        }
                                        SmtRelationOp::LessOrEqual => {
                                            dom.max_val = dom.max_val.min(rhs_bound);
                                        }
                                        SmtRelationOp::Equal => {
                                            dom.min_val = dom.min_val.max(rhs_bound);
                                            dom.max_val = dom.max_val.min(rhs_bound);
                                            match rhs_bound != 0 {
                                                true => dom.excludes_zero = true,
                                                false => {}
                                            }
                                        }
                                        SmtRelationOp::NotEqual => match rhs_bound {
                                            0 => {
                                                dom.excludes_zero = true;
                                                match dom.min_val == 0 {
                                                    true => dom.min_val = 1,
                                                    false => {}
                                                }
                                            }
                                            val => match dom.min_val == val && dom.max_val == val {
                                                true => dom.min_val = val.saturating_add(1),
                                                false => {}
                                            },
                                        },
                                    }

                                    self.domains[slot] = Some(dom);
                                    match dom.is_contradictory() {
                                        true => return false,
                                        false => {}
                                    }
                                    match dom != prev {
                                        true => changed = true,
                                        false => {}
                                    }
                                }
                                None => {}
                            },
                            None => {}
                        }
                    }
                    None => {}
                }
                c_idx += 1;
            }

            match changed {
                false => break,
                true => {}
            }
        }
        true
    }

    pub fn lookup_domain(&self, var_hash: u32) -> Option<SmtDomainInterval> {
        let mut i = 0usize;
        while i < self.domain_count {
            match self.domains[i] {
                Some(dom) => match dom.var_hash == var_hash {
                    true => return Some(dom),
                    false => {}
                },
                None => {}
            }
            i += 1;
        }
        None
    }

    fn lookup_domain_min(&self, var_hash: u32) -> Option<i128> {
        self.lookup_domain(var_hash).map(|d| d.min_val)
    }

    pub fn check_zero_division_satisfiable(&mut self, denom_var_hash: u32) -> SmtSatResult {
        let _ = self.ensure_variable(denom_var_hash);
        match self.propagate_bounds() {
            false => return SmtSatResult::Unsatisfiable,
            true => {}
        }

        match self.lookup_domain(denom_var_hash) {
            Some(dom) => match dom.can_be_zero() {
                true => SmtSatResult::Satisfiable { witness_value: 0 },
                false => SmtSatResult::Unsatisfiable,
            },
            None => SmtSatResult::Satisfiable { witness_value: 0 },
        }
    }

    pub fn check_underflow_satisfiable(
        &mut self,
        lhs_var_hash: u32,
        rhs_var_hash: u32,
    ) -> SmtSatResult {
        let _ = self.ensure_variable(lhs_var_hash);
        let _ = self.ensure_variable(rhs_var_hash);
        match self.propagate_bounds() {
            false => return SmtSatResult::Unsatisfiable,
            true => {}
        }

        let lhs_dom = self
            .lookup_domain(lhs_var_hash)
            .unwrap_or_else(|| SmtDomainInterval::unbounded(lhs_var_hash));
        let rhs_dom = self
            .lookup_domain(rhs_var_hash)
            .unwrap_or_else(|| SmtDomainInterval::unbounded(rhs_var_hash));

        let mut has_explicit_ge_guard = false;
        let mut i = 0usize;
        while i < self.constraint_count {
            match self.constraints[i] {
                Some(c) => {
                    let matches_pair = c.lhs_var_hash == lhs_var_hash
                        && c.rhs_var_hash == Some(rhs_var_hash)
                        && matches!(
                            c.op,
                            SmtRelationOp::GreaterOrEqual | SmtRelationOp::GreaterThan
                        );
                    match matches_pair {
                        true => {
                            has_explicit_ge_guard = true;
                            break;
                        }
                        false => {}
                    }
                }
                None => {}
            }
            i += 1;
        }

        match has_explicit_ge_guard || lhs_dom.min_val >= rhs_dom.max_val {
            true => SmtSatResult::Unsatisfiable,
            false => SmtSatResult::Satisfiable {
                witness_value: lhs_dom.min_val.saturating_sub(1),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefutationRound {
    Round1DimensionalTopology,
    Round2SymmetryCommutatorBarrier,
    Round3SmtCounterexampleProbe,
    Round4ConservationLawCheck,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefutationVerdict {
    SurvivedAdversary {
        proof_seal_crc32: u32,
        rounds_survived: u8,
        witness_value: i128,
    },
    RefutedByAdversary {
        failed_round: RefutationRound,
        reason: &'static str,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MathematicalAdversaryEngine;

impl MathematicalAdversaryEngine {
    pub const fn new() -> Self {
        Self
    }

    pub fn challenge_theorem(
        &self,
        dimension_valid: bool,
        symmetry_preserved: bool,
        denom_var_hash: Option<u32>,
        conservation_delta_bps: u32,
        prover: &mut DeterministicSmtProver,
    ) -> RefutationVerdict {
        match dimension_valid {
            false => {
                return RefutationVerdict::RefutedByAdversary {
                    failed_round: RefutationRound::Round1DimensionalTopology,
                    reason: "Round 1 Refuted: Manifold or matrix dimension mismatch",
                }
            }
            true => {}
        }

        match symmetry_preserved {
            false => {
                return RefutationVerdict::RefutedByAdversary {
                    failed_round: RefutationRound::Round2SymmetryCommutatorBarrier,
                    reason: "Round 2 Refuted: Operator violates Hermitian or topological symmetry",
                }
            }
            true => {}
        }

        let mut witness = 1i128;
        match denom_var_hash {
            Some(d_hash) => match prover.check_zero_division_satisfiable(d_hash) {
                SmtSatResult::Satisfiable { witness_value: _ } => {
                    return RefutationVerdict::RefutedByAdversary {
                        failed_round: RefutationRound::Round3SmtCounterexampleProbe,
                        reason: "Round 3 Refuted: SMT interval solver found zero-divisor singularity",
                    };
                }
                SmtSatResult::Unsatisfiable => {
                    witness = witness_value_non_zero(0);
                }
            },
            None => {}
        }

        match conservation_delta_bps > 10 {
            true => {
                return RefutationVerdict::RefutedByAdversary {
                    failed_round: RefutationRound::Round4ConservationLawCheck,
                    reason: "Round 4 Refuted: Unitary trace or Euler characteristic conservation violated",
                }
            }
            false => {}
        }

        let seal_bytes = [
            (conservation_delta_bps).to_le_bytes(),
            (witness as u32).to_le_bytes(),
        ];
        let flat = [
            seal_bytes[0][0], seal_bytes[0][1], seal_bytes[0][2], seal_bytes[0][3],
            seal_bytes[1][0], seal_bytes[1][1], seal_bytes[1][2], seal_bytes[1][3],
        ];
        RefutationVerdict::SurvivedAdversary {
            proof_seal_crc32: calculate_crc32(&flat),
            rounds_survived: 4,
            witness_value: witness,
        }
    }
}

const fn witness_value_non_zero(v: i128) -> i128 {
    v
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum EngineError {
    BudgetExceeded { limit: u32, current: u32 },
    QuotaExceeded { limit: u64, current: u64 },
    MaxEpochsExceeded { limit: u32, current: u32 },
    InvalidTransition { from: EngineState, to: EngineState },
    IllegalTransition { from: EngineState, event: &'static str },
    TerminalState(EngineState),
    TerminalStateReached { state: EngineState },
    ValidationFailed { reason: &'static str },
    ExecutionFailed { reason: &'static str },
    StorageFull { limit: usize },
    PayloadTooLarge,
    HandlerNotFound(&'static str),
    RegistryFull { limit: usize },
    BufferTooSmall { required: usize, provided: usize },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion(u16),
    CheckpointChecksumMismatch { expected: u32, found: u32 },
    InvalidStateDiscriminant(u8),
    CorruptedCheckpoint { reason: &'static str },
    PlanGraphFull { limit: usize },
    InvalidDependency { node_id: usize, dep_id: usize },
    NodeNotFound(usize),
    InvalidNodeState {
        id: usize,
        from: plan::NodeStatus,
        to: plan::NodeStatus,
    },
    PlanDeadlock { reason: &'static str },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::BudgetExceeded { limit, current } => write!(
                f,
                "Execution budget exceeded: limit {}, reached {}",
                limit, current
            ),
            EngineError::QuotaExceeded { limit, current } => write!(
                f,
                "Compute quota exceeded: limit {}, required {}",
                limit, current
            ),
            EngineError::MaxEpochsExceeded { limit, current } => write!(
                f,
                "Max epochs exceeded: limit {}, reached {}",
                limit, current
            ),
            EngineError::InvalidTransition { from, to } => write!(
                f,
                "Invalid state transition from {:?} to {:?}",
                from, to
            ),
            EngineError::IllegalTransition { from, event } => write!(
                f,
                "Illegal transition from {:?} via {}",
                from, event
            ),
            EngineError::TerminalState(state) => write!(f, "Terminal state {:?} reached", state),
            EngineError::TerminalStateReached { state } => {
                write!(f, "Already in terminal state {:?}", state)
            }
            EngineError::ValidationFailed { reason } => write!(f, "Validation failed: {}", reason),
            EngineError::ExecutionFailed { reason } => write!(f, "Execution failed: {}", reason),
            EngineError::StorageFull { limit } => write!(f, "Storage full: limit {}", limit),
            EngineError::PayloadTooLarge => write!(f, "Payload exceeds maximum permitted size"),
            EngineError::HandlerNotFound(action) => write!(f, "Handler not found: {}", action),
            EngineError::RegistryFull { limit } => write!(f, "Registry full: limit {}", limit),
            EngineError::BufferTooSmall { required, provided } => write!(
                f,
                "Buffer too small: required {}, provided {}",
                required, provided
            ),
            EngineError::InvalidCheckpointMagic => write!(f, "Invalid checkpoint magic"),
            EngineError::UnsupportedCheckpointVersion(v) => {
                write!(f, "Unsupported checkpoint version: {}", v)
            }
            EngineError::CheckpointChecksumMismatch { expected, found } => write!(
                f,
                "Checksum mismatch: expected {:#010X}, found {:#010X}",
                expected, found
            ),
            EngineError::InvalidStateDiscriminant(d) => {
                write!(f, "Invalid state discriminant: {}", d)
            }
            EngineError::CorruptedCheckpoint { reason } => {
                write!(f, "Corrupted checkpoint: {}", reason)
            }
            EngineError::PlanGraphFull { limit } => write!(f, "Plan graph full: limit {}", limit),
            EngineError::InvalidDependency { node_id, dep_id } => write!(
                f,
                "Node {} references invalid dependency {}",
                node_id, dep_id
            ),
            EngineError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            EngineError::InvalidNodeState { id, from, to } => write!(
                f,
                "Invalid status transition for node {}: {:?} -> {:?}",
                id, from, to
            ),
            EngineError::PlanDeadlock { reason } => write!(f, "Plan deadlock: {}", reason),
        }
    }
}

impl std::error::Error for EngineError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EngineState {
    Idle,
    Planning,
    Executing,
    Verifying,
    Completed,
    Failed,
}

impl EngineState {
    pub const fn to_u8(self) -> u8 {
        match self {
            EngineState::Idle => 0,
            EngineState::Planning => 1,
            EngineState::Executing => 2,
            EngineState::Verifying => 3,
            EngineState::Completed => 4,
            EngineState::Failed => 5,
        }
    }

    pub const fn from_u8(val: u8) -> Result<Self, EngineError> {
        match val {
            0 => Ok(EngineState::Idle),
            1 => Ok(EngineState::Planning),
            2 => Ok(EngineState::Executing),
            3 => Ok(EngineState::Verifying),
            4 => Ok(EngineState::Completed),
            5 => Ok(EngineState::Failed),
            other => Err(EngineError::InvalidStateDiscriminant(other)),
        }
    }

    pub const fn is_terminal(&self) -> bool {
        matches!(self, EngineState::Completed | EngineState::Failed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitionRecord {
    pub tick: u64,
    pub step: u32,
    pub from: EngineState,
    pub to: EngineState,
    pub event_tag: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RingBufferAuditLog<const CAP: usize> {
    pub records: [Option<TransitionRecord>; CAP],
    pub head: usize,
    pub total_records: usize,
}

impl<const CAP: usize> Default for RingBufferAuditLog<CAP> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const CAP: usize> RingBufferAuditLog<CAP> {
    pub const fn new() -> Self {
        Self {
            records: [None; CAP],
            head: 0,
            total_records: 0,
        }
    }

    pub fn push(&mut self, record: TransitionRecord) {
        self.records[self.head] = Some(record);
        self.head = (self.head + 1) % CAP;
        self.total_records = self.total_records.saturating_add(1);
    }

    pub const fn len(&self) -> usize {
        match self.total_records < CAP {
            true => self.total_records,
            false => CAP,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.total_records == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundedStorage {
    pub slots: [Option<([u8; MAX_KEY_SIZE], usize, [u8; MAX_VALUE_SIZE], usize)>; MAX_STORAGE_SLOTS],
    pub count: usize,
}

impl Default for BoundedStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl BoundedStorage {
    pub const fn new() -> Self {
        Self {
            slots: [None; MAX_STORAGE_SLOTS],
            count: 0,
        }
    }

    #[inline(always)]
    pub const fn snapshot(&self) -> Self {
        *self
    }

    #[inline(always)]
    pub fn restore(&mut self, snapshot: Self) {
        *self = snapshot;
    }

    pub fn set(&mut self, key: &[u8], value: &[u8]) -> Result<(), EngineError> {
        match key.len() > MAX_KEY_SIZE || value.len() > MAX_VALUE_SIZE {
            true => return Err(EngineError::PayloadTooLarge),
            false => {}
        }

        let mut i = 0usize;
        while i < MAX_STORAGE_SLOTS {
            match &mut self.slots[i] {
                Some((k_buf, k_len, v_buf, v_len)) => match &k_buf[..*k_len] == key {
                    true => {
                        v_buf[..value.len()].copy_from_slice(value);
                        *v_len = value.len();
                        return Ok(());
                    }
                    false => {}
                },
                None => {}
            }
            i += 1;
        }

        match self.count >= MAX_STORAGE_SLOTS {
            true => {
                return Err(EngineError::StorageFull {
                    limit: MAX_STORAGE_SLOTS,
                })
            }
            false => {}
        }

        let mut j = 0usize;
        while j < MAX_STORAGE_SLOTS {
            match self.slots[j] {
                None => {
                    let mut k_buf = [0u8; MAX_KEY_SIZE];
                    let mut v_buf = [0u8; MAX_VALUE_SIZE];
                    k_buf[..key.len()].copy_from_slice(key);
                    v_buf[..value.len()].copy_from_slice(value);
                    self.slots[j] = Some((k_buf, key.len(), v_buf, value.len()));
                    self.count += 1;
                    return Ok(());
                }
                Some(_) => {}
            }
            j += 1;
        }

        Err(EngineError::StorageFull {
            limit: MAX_STORAGE_SLOTS,
        })
    }

    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let mut i = 0usize;
        while i < MAX_STORAGE_SLOTS {
            match &self.slots[i] {
                Some(slot) => match &slot.0[..slot.1] == key {
                    true => return Some(&slot.2[..slot.3]),
                    false => {}
                },
                None => {}
            }
            i += 1;
        }
        None
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

pub type SimpleActionHandlerFn = fn(&[u8], &mut BoundedStorage) -> Result<(), EngineError>;

#[derive(Clone, Copy)]
pub struct ActionRegistry {
    handlers: [Option<(&'static str, SimpleActionHandlerFn)>; MAX_REGISTERED_ACTIONS],
    count: usize,
}

impl Default for ActionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ActionRegistry {
    pub const fn new() -> Self {
        Self {
            handlers: [None; MAX_REGISTERED_ACTIONS],
            count: 0,
        }
    }

    pub fn register(
        &mut self,
        action_type: &'static str,
        handler: SimpleActionHandlerFn,
    ) -> Result<(), EngineError> {
        let mut i = 0usize;
        while i < MAX_REGISTERED_ACTIONS {
            match &mut self.handlers[i] {
                Some((existing_type, existing_handler)) => match *existing_type == action_type {
                    true => {
                        *existing_handler = handler;
                        return Ok(());
                    }
                    false => {}
                },
                None => {}
            }
            i += 1;
        }

        match self.count >= MAX_REGISTERED_ACTIONS {
            true => {
                return Err(EngineError::RegistryFull {
                    limit: MAX_REGISTERED_ACTIONS,
                })
            }
            false => {}
        }

        let mut j = 0usize;
        while j < MAX_REGISTERED_ACTIONS {
            match self.handlers[j] {
                None => {
                    self.handlers[j] = Some((action_type, handler));
                    self.count += 1;
                    return Ok(());
                }
                Some(_) => {}
            }
            j += 1;
        }

        Err(EngineError::RegistryFull {
            limit: MAX_REGISTERED_ACTIONS,
        })
    }

    pub fn dispatch(
        &self,
        action_type: &'static str,
        payload: &[u8],
        storage: &mut BoundedStorage,
    ) -> Result<(), EngineError> {
        let mut i = 0usize;
        while i < MAX_REGISTERED_ACTIONS {
            match self.handlers[i] {
                Some((reg_type, handler)) => match reg_type == action_type {
                    true => return handler(payload, storage),
                    false => {}
                },
                None => {}
            }
            i += 1;
        }
        Err(EngineError::HandlerNotFound(action_type))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unchecked;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Validated;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActionProposal<Stage> {
    action_type: &'static str,
    payload: [u8; MAX_VALUE_SIZE],
    payload_len: usize,
    marker: PhantomData<Stage>,
}

impl ActionProposal<Unchecked> {
    pub fn new(action_type: &'static str, raw_payload: &[u8]) -> Result<Self, EngineError> {
        match raw_payload.len() > MAX_VALUE_SIZE {
            true => Err(EngineError::PayloadTooLarge),
            false => {
                let mut payload = [0u8; MAX_VALUE_SIZE];
                payload[..raw_payload.len()].copy_from_slice(raw_payload);
                Ok(Self {
                    action_type,
                    payload,
                    payload_len: raw_payload.len(),
                    marker: PhantomData,
                })
            }
        }
    }

    pub const fn validate(
        self,
        min_payload_size: usize,
    ) -> Result<ActionProposal<Validated>, EngineError> {
        match self.payload_len < min_payload_size {
            true => Err(EngineError::ValidationFailed {
                reason: "Payload smaller than minimum required length",
            }),
            false => Ok(ActionProposal {
                action_type: self.action_type,
                payload: self.payload,
                payload_len: self.payload_len,
                marker: PhantomData,
            }),
        }
    }
}

impl ActionProposal<Validated> {
    pub const fn action_type(&self) -> &'static str {
        self.action_type
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }
}

struct ByteWriter<'a> {
    buf: &'a mut [u8],
    offset: usize,
}

impl<'a> ByteWriter<'a> {
    fn new(buf: &'a mut [u8]) -> Self {
        Self { buf, offset: 0 }
    }

    fn written_bytes(&self) -> &[u8] {
        &self.buf[..self.offset]
    }

    fn write_bytes(&mut self, data: &[u8]) -> Result<(), EngineError> {
        let end = self.offset + data.len();
        match end > self.buf.len() {
            true => Err(EngineError::BufferTooSmall {
                required: end,
                provided: self.buf.len(),
            }),
            false => {
                self.buf[self.offset..end].copy_from_slice(data);
                self.offset = end;
                Ok(())
            }
        }
    }

    fn write_u8(&mut self, val: u8) -> Result<(), EngineError> {
        self.write_bytes(&[val])
    }

    fn write_u16(&mut self, val: u16) -> Result<(), EngineError> {
        self.write_bytes(&val.to_le_bytes())
    }

    fn write_u32(&mut self, val: u32) -> Result<(), EngineError> {
        self.write_bytes(&val.to_le_bytes())
    }

    fn write_u64(&mut self, val: u64) -> Result<(), EngineError> {
        self.write_bytes(&val.to_le_bytes())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointMetadata {
    pub version: u16,
    pub state: EngineState,
    pub epoch: u32,
    pub tick: u64,
    pub prng_state: u64,
    pub consumed_steps: u32,
    pub max_steps: u32,
    pub consumed_units: u64,
    pub max_units: u64,
    pub total_records: usize,
    pub storage_slots_used: usize,
    pub checksum: u32,
}

impl CheckpointMetadata {
    pub fn inspect(data: &[u8]) -> Result<Self, EngineError> {
        match data.len() < CHECKPOINT_SIZE {
            true => {
                return Err(EngineError::BufferTooSmall {
                    required: CHECKPOINT_SIZE,
                    provided: data.len(),
                })
            }
            false => {}
        }

        let payload_len = CHECKPOINT_SIZE - 4;
        let expected_checksum = u32::from_le_bytes([
            data[payload_len],
            data[payload_len + 1],
            data[payload_len + 2],
            data[payload_len + 3],
        ]);
        let calculated_checksum = calculate_crc32(&data[..payload_len]);

        match expected_checksum != calculated_checksum {
            true => {
                return Err(EngineError::CheckpointChecksumMismatch {
                    expected: expected_checksum,
                    found: calculated_checksum,
                })
            }
            false => {}
        }

        match &data[0..8] == CHECKPOINT_MAGIC {
            true => {}
            false => return Err(EngineError::InvalidCheckpointMagic),
        }

        let version = u16::from_le_bytes([data[8], data[9]]);
        match version == CHECKPOINT_VERSION {
            true => {}
            false => return Err(EngineError::UnsupportedCheckpointVersion(version)),
        }

        let state = EngineState::from_u8(data[10])?;
        let epoch = u32::from_le_bytes([data[11], data[12], data[13], data[14]]);
        let tick = u64::from_le_bytes([
            data[15], data[16], data[17], data[18], data[19], data[20], data[21], data[22],
        ]);
        let prng_state = u64::from_le_bytes([
            data[23], data[24], data[25], data[26], data[27], data[28], data[29], data[30],
        ]);
        let max_steps = u32::from_le_bytes([data[31], data[32], data[33], data[34]]);
        let max_units = u64::from_le_bytes([
            data[35], data[36], data[37], data[38], data[39], data[40], data[41], data[42],
        ]);
        let consumed_steps = u32::from_le_bytes([data[83], data[84], data[85], data[86]]);
        let consumed_units = u64::from_le_bytes([
            data[87], data[88], data[89], data[90], data[91], data[92], data[93], data[94],
        ]);
        let total_records = u64::from_le_bytes([
            data[95], data[96], data[97], data[98], data[99], data[100], data[101], data[102],
        ]) as usize;
        let storage_slots_used = u16::from_le_bytes([data[3185], data[3186]]) as usize;

        Ok(Self {
            version,
            state,
            epoch,
            tick,
            prng_state,
            consumed_steps,
            max_steps,
            consumed_units,
            max_units,
            total_records,
            storage_slots_used,
            checksum: expected_checksum,
        })
    }
}

pub struct DeterministicEngine {
    state: EngineState,
    clock: LogicalClock,
    meter: StepMeter,
    prng: Xorshift64Star,
    audit_log: RingBufferAuditLog<AUDIT_LOG_CAPACITY>,
    storage: BoundedStorage,
    registry: ActionRegistry,
}

impl DeterministicEngine {
    pub const fn new(max_steps: u32) -> Self {
        Self::with_seed_and_meter_config(DEFAULT_PRNG_SEED, MeterConfig::standard(max_steps))
    }

    pub const fn with_seed_and_meter_config(seed: u64, config: MeterConfig) -> Self {
        Self {
            state: EngineState::Idle,
            clock: LogicalClock::new(),
            meter: StepMeter::new(config),
            prng: Xorshift64Star::new(seed),
            audit_log: RingBufferAuditLog::new(),
            storage: BoundedStorage::new(),
            registry: ActionRegistry::new(),
        }
    }

    pub fn register_action(
        &mut self,
        action_type: &'static str,
        handler: SimpleActionHandlerFn,
    ) -> Result<(), EngineError> {
        self.registry.register(action_type, handler)
    }

    pub const fn state(&self) -> EngineState {
        self.state
    }

    pub const fn clock(&self) -> &LogicalClock {
        &self.clock
    }

    pub const fn meter(&self) -> &StepMeter {
        &self.meter
    }

    pub fn meter_mut(&mut self) -> &mut StepMeter {
        &mut self.meter
    }

    pub const fn storage(&self) -> &BoundedStorage {
        &self.storage
    }

    pub fn storage_mut(&mut self) -> &mut BoundedStorage {
        &mut self.storage
    }

    pub fn apply_transition(
        &mut self,
        next_state: EngineState,
        event_tag: &'static str,
    ) -> Result<(), EngineError> {
        match self.state.is_terminal() {
            true => Err(EngineError::TerminalState(self.state)),
            false => {
                self.meter.charge_step()?;
                self.meter.charge_transition()?;
                let tick = self.clock.advance_tick();

                let record = TransitionRecord {
                    tick,
                    step: self.meter.consumed_steps(),
                    from: self.state,
                    to: next_state,
                    event_tag,
                };

                self.audit_log.push(record);
                self.state = next_state;
                Ok(())
            }
        }
    }

    pub fn trigger_planning(&mut self) -> Result<(), EngineError> {
        match self.state {
            EngineState::Idle | EngineState::Verifying => {
                self.clock.advance_epoch();
                self.apply_transition(EngineState::Planning, "TriggerPlanning")
            }
            EngineState::Planning => Ok(()),
            invalid => Err(EngineError::IllegalTransition {
                from: invalid,
                event: "TriggerPlanning",
            }),
        }
    }

    pub fn execute_proposal(
        &mut self,
        proposal: ActionProposal<Validated>,
    ) -> Result<(), EngineError> {
        match self.state {
            EngineState::Planning => {
                let storage_checkpoint = self.storage.snapshot();
                let prng_checkpoint = self.prng;

                self.apply_transition(EngineState::Executing, "DispatchAction")?;

                let execution_result = (|| -> Result<(), EngineError> {
                    self.meter.charge_storage_write(
                        b"last_executed_type".len() + proposal.action_type().len(),
                    )?;
                    self.storage
                        .set(b"last_executed_type", proposal.action_type().as_bytes())?;
                    self.registry.dispatch(
                        proposal.action_type(),
                        proposal.payload(),
                        &mut self.storage,
                    )?;
                    self.apply_transition(EngineState::Verifying, "ActionExecutionComplete")?;
                    Ok(())
                })();

                match execution_result {
                    Ok(()) => Ok(()),
                    Err(err) => {
                        self.storage.restore(storage_checkpoint);
                        self.prng = prng_checkpoint;
                        let tick = self.clock.advance_tick();
                        self.audit_log.push(TransitionRecord {
                            tick,
                            step: self.meter.consumed_steps(),
                            from: EngineState::Executing,
                            to: EngineState::Planning,
                            event_tag: "RollbackOnFailure",
                        });
                        self.state = EngineState::Planning;
                        Err(err)
                    }
                }
            }
            invalid => Err(EngineError::IllegalTransition {
                from: invalid,
                event: "DispatchAction",
            }),
        }
    }

    pub fn finalize_verification(&mut self, goal_reached: bool) -> Result<(), EngineError> {
        match (self.state, goal_reached) {
            (EngineState::Verifying, true) => {
                self.apply_transition(EngineState::Completed, "VerificationPassed")
            }
            (EngineState::Verifying, false) => {
                self.clock.advance_epoch();
                self.apply_transition(EngineState::Planning, "VerificationFailedRetry")
            }
            (invalid, _) => Err(EngineError::IllegalTransition {
                from: invalid,
                event: "FinalizeVerification",
            }),
        }
    }

    pub fn export_checkpoint(&self, out: &mut [u8]) -> Result<usize, EngineError> {
        match out.len() < CHECKPOINT_SIZE {
            true => {
                return Err(EngineError::BufferTooSmall {
                    required: CHECKPOINT_SIZE,
                    provided: out.len(),
                })
            }
            false => {}
        }

        let mut writer = ByteWriter::new(out);
        writer.write_bytes(CHECKPOINT_MAGIC)?;
        writer.write_u16(CHECKPOINT_VERSION)?;
        writer.write_u8(self.state.to_u8())?;
        writer.write_u32(self.clock.epoch())?;
        writer.write_u64(self.clock.tick())?;
        writer.write_u64(self.prng.state())?;

        let cfg = self.meter.config();
        writer.write_u32(cfg.max_steps)?;
        writer.write_u64(cfg.max_units)?;
        writer.write_u64(cfg.step_cost)?;
        writer.write_u64(cfg.transition_cost)?;
        writer.write_u64(cfg.storage_read_byte_cost)?;
        writer.write_u64(cfg.storage_write_byte_cost)?;
        writer.write_u64(cfg.prng_cost)?;

        writer.write_u32(self.meter.consumed_steps())?;
        writer.write_u64(self.meter.consumed_units())?;

        writer.write_u64(self.audit_log.total_records as u64)?;
        writer.write_u64(self.audit_log.head as u64)?;
        writer.write_u16(AUDIT_LOG_CAPACITY as u16)?;

        let mut r_idx = 0usize;
        while r_idx < AUDIT_LOG_CAPACITY {
            match self.audit_log.records[r_idx] {
                Some(record) => {
                    writer.write_u8(1)?;
                    writer.write_u64(record.tick)?;
                    writer.write_u32(record.step)?;
                    writer.write_u8(record.from.to_u8())?;
                    writer.write_u8(record.to.to_u8())?;

                    let tag_bytes = record.event_tag.as_bytes();
                    let tag_len = tag_bytes.len().min(MAX_EVENT_TAG_SIZE);
                    writer.write_u8(tag_len as u8)?;

                    let mut tag_buf = [0u8; MAX_EVENT_TAG_SIZE];
                    tag_buf[..tag_len].copy_from_slice(&tag_bytes[..tag_len]);
                    writer.write_bytes(&tag_buf)?;
                }
                None => {
                    writer.write_u8(0)?;
                    writer.write_u64(0)?;
                    writer.write_u32(0)?;
                    writer.write_u8(0)?;
                    writer.write_u8(0)?;
                    writer.write_u8(0)?;
                    writer.write_bytes(&[0u8; MAX_EVENT_TAG_SIZE])?;
                }
            }
            r_idx += 1;
        }

        writer.write_u16(self.storage.count as u16)?;
        writer.write_u16(MAX_STORAGE_SLOTS as u16)?;

        let mut s_idx = 0usize;
        while s_idx < MAX_STORAGE_SLOTS {
            match &self.storage.slots[s_idx] {
                Some((k_buf, k_len, v_buf, v_len)) => {
                    writer.write_u8(1)?;
                    writer.write_u8(*k_len as u8)?;
                    writer.write_bytes(k_buf)?;
                    writer.write_u16(*v_len as u16)?;
                    writer.write_bytes(v_buf)?;
                }
                None => {
                    writer.write_u8(0)?;
                    writer.write_u8(0)?;
                    writer.write_bytes(&[0u8; MAX_KEY_SIZE])?;
                    writer.write_u16(0)?;
                    writer.write_bytes(&[0u8; MAX_VALUE_SIZE])?;
                }
            }
            s_idx += 1;
        }

        let checksum = calculate_crc32(writer.written_bytes());
        writer.write_u32(checksum)?;

        Ok(CHECKPOINT_SIZE)
    }

    pub fn export_checkpoint_array(&self) -> [u8; CHECKPOINT_SIZE] {
        let mut buf = [0u8; CHECKPOINT_SIZE];
        let _ = self.export_checkpoint(&mut buf);
        buf
    }
}