//! Stack-allocated 16-node Directed Acyclic Graph (DAG) for multi-stage mathematical derivations.

use crate::metering::StepMeter;
use crate::{EngineError, MAX_VALUE_SIZE};

pub const MAX_PLAN_NODES: usize = 16;
pub const MAX_DEPENDENCIES: usize = 4;
pub const MAX_NODE_PAYLOAD_SIZE: usize = MAX_VALUE_SIZE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    Succeeded,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanNode {
    pub id: usize,
    pub action_type: &'static str,
    payload: [u8; MAX_NODE_PAYLOAD_SIZE],
    payload_len: usize,
    pub status: NodeStatus,
    pub dependencies: [usize; MAX_DEPENDENCIES],
    pub dependency_count: usize,
}

impl PlanNode {
    pub fn payload(&self) -> &[u8] {
        &self.payload[..self.payload_len]
    }

    pub fn dependencies(&self) -> &[usize] {
        &self.dependencies[..self.dependency_count]
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PlanGraph {
    nodes: [Option<PlanNode>; MAX_PLAN_NODES],
    count: usize,
}

impl Default for PlanGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanGraph {
    pub const fn new() -> Self {
        Self {
            nodes: [None; MAX_PLAN_NODES],
            count: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.count
    }

    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    pub fn add_node(
        &mut self,
        action_type: &'static str,
        payload: &[u8],
        dependencies: &[usize],
    ) -> Result<usize, EngineError> {
        match self.count >= MAX_PLAN_NODES {
            true => {
                return Err(EngineError::PlanGraphFull {
                    limit: MAX_PLAN_NODES,
                })
            }
            false => {}
        }

        match payload.len() > MAX_NODE_PAYLOAD_SIZE {
            true => return Err(EngineError::PayloadTooLarge),
            false => {}
        }

        match dependencies.len() > MAX_DEPENDENCIES {
            true => {
                return Err(EngineError::InvalidDependency {
                    node_id: self.count,
                    dep_id: 0,
                })
            }
            false => {}
        }

        let mut d_idx = 0usize;
        while d_idx < dependencies.len() {
            let dep = dependencies[d_idx];
            match dep >= self.count {
                true => {
                    return Err(EngineError::InvalidDependency {
                        node_id: self.count,
                        dep_id: dep,
                    })
                }
                false => {}
            }
            d_idx += 1;
        }

        let mut payload_buf = [0u8; MAX_NODE_PAYLOAD_SIZE];
        payload_buf[..payload.len()].copy_from_slice(payload);

        let mut deps_buf = [0usize; MAX_DEPENDENCIES];
        deps_buf[..dependencies.len()].copy_from_slice(dependencies);

        let initial_status = match dependencies.is_empty() {
            true => NodeStatus::Ready,
            false => NodeStatus::Pending,
        };

        let node_id = self.count;
        self.nodes[node_id] = Some(PlanNode {
            id: node_id,
            action_type,
            payload: payload_buf,
            payload_len: payload.len(),
            status: initial_status,
            dependencies: deps_buf,
            dependency_count: dependencies.len(),
        });

        self.count += 1;
        Ok(node_id)
    }

    pub fn get_node(&self, id: usize) -> Option<&PlanNode> {
        match id < self.count {
            true => self.nodes[id].as_ref(),
            false => None,
        }
    }

    pub fn get_node_mut(&mut self, id: usize) -> Option<&mut PlanNode> {
        match id < self.count {
            true => self.nodes[id].as_mut(),
            false => None,
        }
    }

    pub fn set_node_status(&mut self, id: usize, status: NodeStatus) -> Result<(), EngineError> {
        let node = self.get_node_mut(id).ok_or(EngineError::NodeNotFound(id))?;
        node.status = status;
        Ok(())
    }

    pub fn resolve_readiness(&mut self, meter: &mut StepMeter) -> Result<usize, EngineError> {
        meter.charge_step()?;
        let mut newly_ready = 0usize;
        let mut changed = true;
        let mut pass_budget = MAX_PLAN_NODES;

        while changed && pass_budget > 0 {
            pass_budget -= 1;
            changed = false;
            let mut i = 0usize;
            while i < self.count {
                let transition = match self.nodes[i].as_ref() {
                    Some(node) => match node.status {
                        NodeStatus::Pending => {
                            let mut any_failed = false;
                            let mut all_succeeded = true;
                            let mut d = 0usize;

                            while d < node.dependency_count {
                                let dep_id = node.dependencies[d];
                                match self.nodes[dep_id].as_ref() {
                                    Some(dep_node) => match dep_node.status {
                                        NodeStatus::Failed | NodeStatus::Skipped => {
                                            any_failed = true;
                                            break;
                                        }
                                        NodeStatus::Succeeded => {}
                                        _ => {
                                            all_succeeded = false;
                                        }
                                    },
                                    None => {}
                                }
                                d += 1;
                            }

                            match (any_failed, all_succeeded) {
                                (true, _) => Some(NodeStatus::Skipped),
                                (false, true) => Some(NodeStatus::Ready),
                                (false, false) => None,
                            }
                        }
                        _ => None,
                    },
                    None => None,
                };

                match transition {
                    Some(new_status) => match self.nodes[i].as_mut() {
                        Some(n) => {
                            n.status = new_status;
                            changed = true;
                            match new_status {
                                NodeStatus::Ready => newly_ready += 1,
                                _ => {}
                            }
                        }
                        None => {}
                    },
                    None => {}
                }
                i += 1;
            }
        }
        Ok(newly_ready)
    }

    pub fn next_ready_node(&self) -> Option<usize> {
        let mut i = 0usize;
        while i < self.count {
            match self.nodes[i].as_ref() {
                Some(node) => match node.status {
                    NodeStatus::Ready => return Some(node.id),
                    _ => {}
                },
                None => {}
            }
            i += 1;
        }
        None
    }

    pub fn is_complete(&self) -> bool {
        match self.count {
            0 => true,
            _ => {
                let mut i = 0usize;
                while i < self.count {
                    match self.nodes[i].as_ref() {
                        Some(node) => match node.status {
                            NodeStatus::Succeeded | NodeStatus::Failed | NodeStatus::Skipped => {}
                            _ => return false,
                        },
                        None => {}
                    }
                    i += 1;
                }
                true
            }
        }
    }
}