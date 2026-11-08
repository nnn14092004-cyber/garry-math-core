//! Deterministic mathematical intent classification, stack ping-pong composite splitting,
//! target `.txt` file extraction, and execution DAG builder.

use crate::ephemeral::compute_problem_bitmask;
use crate::plan::PlanGraph;
use crate::{EngineError, EngineState};

pub const MAX_STAGE_SPLITS: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathIntentKind {
    PureCompute,
    ComputeAndExportTxt,
    MultiStagePipeline,
    StoreAxiom,
    SystemInspect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FsmSnapshot {
    pub state: EngineState,
    pub epoch: u32,
    pub gas_consumed: u64,
    pub steps_consumed: u32,
    pub checkpoint_crc32: u32,
    pub storage_slots_used: usize,
}

impl Default for FsmSnapshot {
    fn default() -> Self {
        Self {
            state: EngineState::Idle,
            epoch: 1,
            gas_consumed: 0,
            steps_consumed: 0,
            checkpoint_crc32: 0,
            storage_slots_used: 0,
        }
    }
}

fn find_case_insensitive_ascii(haystack: &str, needle: &str) -> Option<(usize, usize)> {
    match needle.is_empty() || haystack.len() < needle.len() {
        true => return None,
        false => {}
    }
    let h_bytes = haystack.as_bytes();
    let n_bytes = needle.as_bytes();
    let max_start = h_bytes.len() - n_bytes.len();
    let mut i = 0usize;

    while i <= max_start {
        let mut matched = true;
        let mut j = 0usize;
        while j < n_bytes.len() {
            let hb = match h_bytes[i + j] >= b'A' && h_bytes[i + j] <= b'Z' {
                true => h_bytes[i + j] + 32,
                false => h_bytes[i + j],
            };
            let nb = match n_bytes[j] >= b'A' && n_bytes[j] <= b'Z' {
                true => n_bytes[j] + 32,
                false => n_bytes[j],
            };
            match hb == nb {
                true => j += 1,
                false => {
                    matched = false;
                    break;
                }
            }
        }
        match matched
            && haystack.is_char_boundary(i)
            && haystack.is_char_boundary(i + n_bytes.len())
        {
            true => return Some((i, i + n_bytes.len())),
            false => i += 1,
        }
    }
    None
}

fn split_slice_into_buffer<'a>(
    segment: &'a str,
    delimiter: &str,
    out: &mut [&'a str; MAX_STAGE_SPLITS],
    out_count: &mut usize,
) {
    let mut remainder = segment;
    let mut found_delimiter = false;
    let mut budget = 16usize;

    while budget > 0 {
        budget -= 1;
        match find_case_insensitive_ascii(remainder, delimiter) {
            Some((start, end)) => match end > start {
                true => {
                    found_delimiter = true;
                    let prefix = remainder[..start].trim();
                    match !prefix.is_empty() && *out_count < MAX_STAGE_SPLITS {
                        true => {
                            out[*out_count] = prefix;
                            *out_count += 1;
                        }
                        false => {}
                    }
                    remainder = &remainder[end..];
                }
                false => break,
            },
            None => break,
        }
    }

    let tail = match found_delimiter {
        true => remainder.trim(),
        false => segment.trim(),
    };
    match !tail.is_empty() && *out_count < MAX_STAGE_SPLITS {
        true => {
            out[*out_count] = tail;
            *out_count += 1;
        }
        false => {}
    }
}

pub fn split_composite_intents_stack(text: &str) -> ([&str; MAX_STAGE_SPLITS], usize) {
    const CONJUNCTIONS: &[&str] = &[" then ", " -> ", " ;; "];
    let mut current_buf: [&str; MAX_STAGE_SPLITS] = [""; MAX_STAGE_SPLITS];

    let clean_init = text.trim();
    let mut current_len = match clean_init.is_empty() {
        true => return (current_buf, 0),
        false => {
            current_buf[0] = clean_init;
            1usize
        }
    };

    let mut c_idx = 0usize;
    while c_idx < CONJUNCTIONS.len() {
        let conj = CONJUNCTIONS[c_idx];
        let mut next_buf: [&str; MAX_STAGE_SPLITS] = [""; MAX_STAGE_SPLITS];
        let mut next_len = 0usize;

        let mut i = 0usize;
        while i < current_len {
            let seg = current_buf[i];
            let is_file_arrow = conj == " -> " && extract_target_filename(seg).is_some();
            match is_file_arrow {
                true => match next_len < MAX_STAGE_SPLITS {
                    true => {
                        next_buf[next_len] = seg;
                        next_len += 1;
                    }
                    false => {}
                },
                false => split_slice_into_buffer(seg, conj, &mut next_buf, &mut next_len),
            }
            i += 1;
        }

        match next_len > 0 {
            true => {
                current_len = next_len;
                let mut k = 0usize;
                while k < next_len {
                    current_buf[k] = next_buf[k];
                    k += 1;
                }
            }
            false => {}
        }
        c_idx += 1;
    }

    (current_buf, current_len)
}

pub fn extract_target_filename(text: &str) -> Option<String> {
    const EXTS: &[&str] = &[".txt", ".md", ".log"];
    let mut budget = 128usize;
    for raw_tok in text.split_whitespace().rev() {
        match budget == 0 {
            true => break,
            false => budget -= 1,
        }
        let clean = raw_tok.trim_matches(|c: char| {
            c == '"'
                || c == '\''
                || c == ','
                || c == ';'
                || c == ':'
                || c == '('
                || c == ')'
                || c == '['
                || c == ']'
        });
        let lower = clean.to_lowercase();
        let mut e_idx = 0usize;
        while e_idx < EXTS.len() {
            let ext = EXTS[e_idx];
            match lower.ends_with(ext) && clean.len() > ext.len() && clean != "task_report.txt" {
                true => return Some(clean.to_string()),
                false => {}
            }
            e_idx += 1;
        }
    }
    None
}

pub fn extract_compute_and_file_params(text: &str) -> (Option<String>, String) {
    let target_opt = extract_target_filename(text);
    let lower = text.to_lowercase();
    const MARKERS: &[&str] = &[
        "and save to file",
        "and export to file",
        "save to file",
        "save to",
        "save file",
        "export to file",
        "export to",
        "export file",
        "write to file",
        "write file",
        "->",
        "=>",
    ];

    let mut clean_query = text.trim();
    let mut m_idx = 0usize;
    while m_idx < MARKERS.len() {
        let marker = MARKERS[m_idx];
        match lower.rfind(marker) {
            Some(pos) => {
                let after = text[pos + marker.len()..].trim();
                let has_ext =
                    after.ends_with(".txt") || after.ends_with(".md") || after.ends_with(".log");
                match has_ext {
                    true => {
                        let candidate = text[..pos]
                            .trim()
                            .trim_end_matches(|c: char| c == ',' || c == ';');
                        match !candidate.is_empty() {
                            true => {
                                clean_query = candidate;
                                break;
                            }
                            false => {}
                        }
                    }
                    false => {}
                }
            }
            None => {}
        }
        m_idx += 1;
    }

    (target_opt, clean_query.to_string())
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeterministicMathPlanner;

impl DeterministicMathPlanner {
    pub const fn new() -> Self {
        Self
    }

    pub fn classify_intent(&self, query: &str) -> MathIntentKind {
        let trimmed = query.trim();
        let (_, stage_cnt) = split_composite_intents_stack(trimmed);
        match stage_cnt >= 2 {
            true => return MathIntentKind::MultiStagePipeline,
            false => {}
        }
        match trimmed.contains(":=") {
            true => return MathIntentKind::StoreAxiom,
            false => {}
        }
        match extract_target_filename(trimmed).is_some() {
            true => MathIntentKind::ComputeAndExportTxt,
            false => MathIntentKind::PureCompute,
        }
    }

    pub fn build_derivation_dag(&self, query: &str) -> Result<PlanGraph, EngineError> {
        let trimmed = query.trim();
        match trimmed.is_empty() {
            true => {
                return Err(EngineError::ValidationFailed {
                    reason: "Query payload cannot be empty",
                })
            }
            false => {}
        }

        let mut graph = PlanGraph::new();
        let (target_file_opt, clean_query) = extract_compute_and_file_params(trimmed);
        let mask = compute_problem_bitmask(&clean_query);
        let mask_bytes = mask.to_le_bytes();

        let n0 = graph.add_node("AXIOM_FUNNEL_GATE", &mask_bytes, &[])?;

        let (stages, stage_cnt) = split_composite_intents_stack(&clean_query);
        let mut prev_node = n0;
        let mut s_idx = 0usize;
        let effective_stages = stage_cnt.max(1).min(6);

        while s_idx < effective_stages {
            let stage_str = match stage_cnt > 0 {
                true => stages[s_idx],
                false => clean_query.as_str(),
            };
            let q_bytes = stage_str.as_bytes();
            let q_len = q_bytes.len().min(256);
            let solver_node =
                graph.add_node("MATH_SOLVER_STAGE", &q_bytes[..q_len], &[prev_node])?;
            prev_node = solver_node;
            s_idx += 1;
        }

        match target_file_opt {
            Some(fname) => {
                let f_bytes = fname.as_bytes();
                let f_len = f_bytes.len().min(256);
                let _ = graph.add_node("FS_EXPORT_TXT", &f_bytes[..f_len], &[prev_node])?;
            }
            None => {}
        }

        Ok(graph)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_planner_extracts_txt_and_builds_3_node_dag() {
        let planner = DeterministicMathPlanner::new();
        let q = "Cholesky [[4, 12], [12, 37]] save to file chol_proof.txt";
        assert_eq!(planner.classify_intent(q), MathIntentKind::ComputeAndExportTxt);
        let dag = planner.build_derivation_dag(q).unwrap();
        assert_eq!(dag.len(), 3);
        assert_eq!(dag.get_node(2).unwrap().action_type, "FS_EXPORT_TXT");
        assert_eq!(dag.get_node(2).unwrap().payload(), b"chol_proof.txt");
    }

    #[test]
    fn test_planner_splits_multi_stage_pipeline_on_stack() {
        let q = "Cholesky [[4, 12], [12, 37]] then Jacobi [[4, 1], [1, 3]] -> multi_stage.txt";
        let (stages, cnt) = split_composite_intents_stack(q);
        assert_eq!(cnt, 2);
        assert!(stages[0].contains("Cholesky"));
        assert!(stages[1].contains("Jacobi"));
    }
}