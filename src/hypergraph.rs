//! L2 256-Axiom associative hypergraph memory engine with vitality decay and L1 cache synchronization.

use crate::crc::calculate_crc32;
use crate::{BoundedStorage, EngineError, MAX_KEY_SIZE, MAX_VALUE_SIZE};

pub const HYPERGRAPH_L2_CAPACITY: usize = 256;
pub const MAX_HYPEREDGES: usize = 64;
pub const L1_HOT_SLOTS: usize = 16;
pub const INITIAL_VITALITY_SCORE: u32 = 1000;
pub const DECAY_EVICTION_THRESHOLD: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HyperGraphAxiomNode {
    pub axiom_id: u32,
    pub key: [u8; MAX_KEY_SIZE],
    pub key_len: usize,
    pub formula: [u8; MAX_VALUE_SIZE],
    pub formula_len: usize,
    pub signature_crc32: u32,
    pub semantic_bitmask: u64,
    pub vitality: u32,
    pub last_active_epoch: u32,
    pub access_hits: u32,
}

impl HyperGraphAxiomNode {
    pub fn new(
        axiom_id: u32,
        key_slice: &[u8],
        formula_slice: &[u8],
        semantic_bitmask: u64,
        epoch: u32,
    ) -> Self {
        let k_len = match key_slice.len() > MAX_KEY_SIZE {
            true => MAX_KEY_SIZE,
            false => key_slice.len(),
        };
        let mut key = [0u8; MAX_KEY_SIZE];
        key[..k_len].copy_from_slice(&key_slice[..k_len]);

        let f_len = match formula_slice.len() > MAX_VALUE_SIZE {
            true => MAX_VALUE_SIZE,
            false => formula_slice.len(),
        };
        let mut formula = [0u8; MAX_VALUE_SIZE];
        formula[..f_len].copy_from_slice(&formula_slice[..f_len]);

        let signature_crc32 = calculate_crc32(&key[..k_len]) ^ calculate_crc32(&formula[..f_len]);

        Self {
            axiom_id,
            key,
            key_len: k_len,
            formula,
            formula_len: f_len,
            signature_crc32,
            semantic_bitmask,
            vitality: INITIAL_VITALITY_SCORE,
            last_active_epoch: epoch,
            access_hits: 1,
        }
    }

    pub fn key_slice(&self) -> &[u8] {
        &self.key[..self.key_len]
    }

    pub fn formula_slice(&self) -> &[u8] {
        &self.formula[..self.formula_len]
    }

    pub const fn effective_resonance_score(&self, problem_mask: u64) -> u64 {
        let overlap_bits = (self.semantic_bitmask & problem_mask).count_ones() as u64;
        let base_vitality = self.vitality as u64;
        let hit_bonus = (self.access_hits as u64).saturating_mul(25);
        let resonance_multiplier = overlap_bits.saturating_mul(250);
        base_vitality
            .saturating_add(hit_bonus)
            .saturating_add(resonance_multiplier)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AssociativeHyperEdge {
    pub node_ids: [u32; 4],
    pub arity: usize,
    pub shared_semantic_mask: u64,
    pub causal_weight_bps: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct AxiomHyperGraphL2 {
    pub nodes: [Option<HyperGraphAxiomNode>; HYPERGRAPH_L2_CAPACITY],
    pub active_nodes: usize,
    pub edges: [Option<AssociativeHyperEdge>; MAX_HYPEREDGES],
    pub edge_count: usize,
    pub current_epoch: u32,
    pub next_axiom_id: u32,
    pub total_decayed_evictions: u64,
}

impl Default for AxiomHyperGraphL2 {
    fn default() -> Self {
        Self::new()
    }
}

impl AxiomHyperGraphL2 {
    pub const fn new() -> Self {
        Self {
            nodes: [None; HYPERGRAPH_L2_CAPACITY],
            active_nodes: 0,
            edges: [None; MAX_HYPEREDGES],
            edge_count: 0,
            current_epoch: 1,
            next_axiom_id: 1,
            total_decayed_evictions: 0,
        }
    }

    pub const fn active_node_count(&self) -> usize {
        self.active_nodes
    }

    pub const fn active_edge_count(&self) -> usize {
        self.edge_count
    }

    pub fn upsert_axiom(
        &mut self,
        key_slice: &[u8],
        formula_slice: &[u8],
        semantic_bitmask: u64,
    ) -> u32 {
        let k_len = match key_slice.len() > MAX_KEY_SIZE {
            true => MAX_KEY_SIZE,
            false => key_slice.len(),
        };
        let target_key = &key_slice[..k_len];

        let mut idx = 0usize;
        while idx < HYPERGRAPH_L2_CAPACITY {
            match &mut self.nodes[idx] {
                Some(node) => match node.key_slice() == target_key {
                    true => {
                        let f_len = match formula_slice.len() > MAX_VALUE_SIZE {
                            true => MAX_VALUE_SIZE,
                            false => formula_slice.len(),
                        };
                        node.formula[..f_len].copy_from_slice(&formula_slice[..f_len]);
                        node.formula_len = f_len;
                        node.semantic_bitmask |= semantic_bitmask;
                        node.vitality = node.vitality.saturating_add(350).min(2500);
                        node.last_active_epoch = self.current_epoch;
                        node.access_hits = node.access_hits.saturating_add(1);
                        let id = node.axiom_id;
                        self.rebuild_resonance_hyperedges();
                        return id;
                    }
                    false => {}
                },
                None => {}
            }
            idx += 1;
        }

        let assigned_id = self.next_axiom_id;
        self.next_axiom_id = self.next_axiom_id.saturating_add(1);
        let new_node = HyperGraphAxiomNode::new(
            assigned_id,
            key_slice,
            formula_slice,
            semantic_bitmask,
            self.current_epoch,
        );

        let mut free_slot: Option<usize> = None;
        let mut weakest_slot = 0usize;
        let mut weakest_vitality = u32::MAX;

        let mut scan = 0usize;
        while scan < HYPERGRAPH_L2_CAPACITY {
            match &self.nodes[scan] {
                None => {
                    free_slot = Some(scan);
                    break;
                }
                Some(node) => match node.vitality < weakest_vitality {
                    true => {
                        weakest_vitality = node.vitality;
                        weakest_slot = scan;
                    }
                    false => {}
                },
            }
            scan += 1;
        }

        match free_slot {
            Some(slot) => {
                self.nodes[slot] = Some(new_node);
                self.active_nodes += 1;
            }
            None => {
                self.nodes[weakest_slot] = Some(new_node);
                self.total_decayed_evictions = self.total_decayed_evictions.saturating_add(1);
            }
        }

        self.rebuild_resonance_hyperedges();
        assigned_id
    }

    pub fn apply_epoch_decay(&mut self, decay_step: u32, active_problem_mask: u64) -> usize {
        self.current_epoch = self.current_epoch.saturating_add(1);
        let mut purged_count = 0usize;
        let mut idx = 0usize;

        while idx < HYPERGRAPH_L2_CAPACITY {
            let mut should_evict = false;
            match &mut self.nodes[idx] {
                Some(node) => {
                    let resonates = (node.semantic_bitmask & active_problem_mask) != 0
                        && active_problem_mask != 0;
                    match resonates {
                        true => {
                            node.vitality = node.vitality.saturating_add(150).min(2500);
                            node.last_active_epoch = self.current_epoch;
                        }
                        false => {
                            node.vitality = node.vitality.saturating_sub(decay_step);
                            match node.vitality < DECAY_EVICTION_THRESHOLD {
                                true => should_evict = true,
                                false => {}
                            }
                        }
                    }
                }
                None => {}
            }

            match should_evict {
                true => {
                    self.nodes[idx] = None;
                    self.active_nodes = self.active_nodes.saturating_sub(1);
                    self.total_decayed_evictions = self.total_decayed_evictions.saturating_add(1);
                    purged_count += 1;
                }
                false => {}
            }
            idx += 1;
        }

        self.rebuild_resonance_hyperedges();
        purged_count
    }

    pub fn rebuild_resonance_hyperedges(&mut self) {
        self.edges = [None; MAX_HYPEREDGES];
        self.edge_count = 0;

        let mut i = 0usize;
        while i < HYPERGRAPH_L2_CAPACITY && self.edge_count < MAX_HYPEREDGES {
            match &self.nodes[i] {
                Some(anchor) => {
                    let mut group_ids = [anchor.axiom_id, 0, 0, 0];
                    let mut arity = 1usize;
                    let mut shared_mask = anchor.semantic_bitmask;

                    let mut j = i + 1;
                    while j < HYPERGRAPH_L2_CAPACITY && arity < 4 {
                        match &self.nodes[j] {
                            Some(peer) => {
                                let common = shared_mask & peer.semantic_bitmask;
                                match common != 0 {
                                    true => {
                                        group_ids[arity] = peer.axiom_id;
                                        arity += 1;
                                        shared_mask = common;
                                    }
                                    false => {}
                                }
                            }
                            None => {}
                        }
                        j += 1;
                    }

                    match arity >= 2 {
                        true => {
                            let weight_bps = (shared_mask.count_ones() * 2500).min(10_000);
                            self.edges[self.edge_count] = Some(AssociativeHyperEdge {
                                node_ids: group_ids,
                                arity,
                                shared_semantic_mask: shared_mask,
                                causal_weight_bps: weight_bps,
                            });
                            self.edge_count += 1;
                        }
                        false => {}
                    }
                }
                None => {}
            }
            i += 1;
        }
    }

    pub fn ingest_from_l1_storage(&mut self, l1: &BoundedStorage, default_semantic_mask: u64) -> usize {
        let mut synced = 0usize;
        let mut s_idx = 0usize;
        while s_idx < L1_HOT_SLOTS {
            match &l1.slots[s_idx] {
                Some((key_buf, key_len, val_buf, val_len)) => {
                    self.upsert_axiom(
                        &key_buf[..*key_len],
                        &val_buf[..*val_len],
                        default_semantic_mask,
                    );
                    synced += 1;
                }
                None => {}
            }
            s_idx += 1;
        }
        synced
    }

    pub fn promote_top_resonating_to_l1(
        &mut self,
        problem_mask: u64,
        max_l1_promotions: usize,
        l1: &mut BoundedStorage,
    ) -> Result<usize, EngineError> {
        let limit = match max_l1_promotions > L1_HOT_SLOTS {
            true => L1_HOT_SLOTS,
            false => max_l1_promotions,
        };

        let mut selected_indices: [Option<usize>; L1_HOT_SLOTS] = [None; L1_HOT_SLOTS];
        let mut selected_scores: [u64; L1_HOT_SLOTS] = [0u64; L1_HOT_SLOTS];
        let mut selected_count = 0usize;

        let mut idx = 0usize;
        while idx < HYPERGRAPH_L2_CAPACITY {
            match &self.nodes[idx] {
                Some(node) => {
                    let score = node.effective_resonance_score(problem_mask);
                    match selected_count < limit {
                        true => {
                            selected_indices[selected_count] = Some(idx);
                            selected_scores[selected_count] = score;
                            selected_count += 1;
                        }
                        false => match limit > 0 {
                            true => {
                                let mut min_pos = 0usize;
                                let mut min_score = selected_scores[0];
                                let mut k = 1usize;
                                while k < selected_count {
                                    match selected_scores[k] < min_score {
                                        true => {
                                            min_score = selected_scores[k];
                                            min_pos = k;
                                        }
                                        false => {}
                                    }
                                    k += 1;
                                }
                                match score > min_score {
                                    true => {
                                        selected_indices[min_pos] = Some(idx);
                                        selected_scores[min_pos] = score;
                                    }
                                    false => {}
                                }
                            }
                            false => {}
                        },
                    }
                }
                None => {}
            }
            idx += 1;
        }

        let mut promoted = 0usize;
        let mut p_idx = 0usize;
        while p_idx < selected_count {
            match selected_indices[p_idx] {
                Some(node_slot) => match &mut self.nodes[node_slot] {
                    Some(node) => {
                        node.access_hits = node.access_hits.saturating_add(1);
                        node.last_active_epoch = self.current_epoch;
                        l1.set(&node.key[..node.key_len], &node.formula[..node.formula_len])?;
                        promoted += 1;
                    }
                    None => {}
                },
                None => {}
            }
            p_idx += 1;
        }

        Ok(promoted)
    }
}