//! Ephemeral axiom synthesis, FNV-1a fast-gate funnel, 256-slot resonance hypergraph,
//! and deterministic L2 knowledge persistence engine.

use std::io::Write;
use std::path::Path;

use crate::{
    calculate_crc32, ActionProposal, AxiomHyperGraphL2, BoundedStorage, DeterministicEngine,
    DeterministicSmtProver, EngineError, EngineState, MathematicalAdversaryEngine,
    RefutationVerdict, SmtConstraint, SmtRelationOp, MAX_KEY_SIZE, MAX_VALUE_SIZE,
};

pub const FNV_OFFSET_BASIS: u32 = 2_166_136_261;
pub const FNV_PRIME: u32 = 16_777_619;
pub const MAX_HASH_SCAN_BYTES: usize = 512;
pub const MAX_ACCEPT_SLOTS: usize = 8;
pub const MAX_TOKEN_SLOTS: usize = 32;
pub const MAX_PIPELINE_WINDOW: usize = 4;
pub const MAX_SLIDE_BUDGET: usize = 8;
pub const SCRATCHPAD_SIZE: usize = 16;
pub const CATALOG_GATE_SIZE: usize = 9;
pub const MAX_HYPERGRAPH_AXIOMS: usize = 256;

pub const MASK_QUANTUM: u64 = 1 << 0;
pub const MASK_DEPHASING: u64 = 1 << 1;
pub const MASK_FACTORIZATION: u64 = 1 << 2;
pub const MASK_SPECTRAL: u64 = 1 << 3;
pub const MASK_POLYHEDRON: u64 = 1 << 4;
pub const MASK_DYNAMIC_STEP: u64 = 1 << 5;
pub const MASK_ECONOMIC_AMM: u64 = 1 << 6;
pub const MASK_AUDIT_CEI: u64 = 1 << 7;
pub const MASK_AUDIT_ARITHMETIC: u64 = 1 << 8;
pub const MASK_SYMPLECTIC_LIE: u64 = 1 << 9;
pub const MASK_RICCI_MANIFOLD: u64 = 1 << 10;
pub const MASK_STOCHASTIC_PDE: u64 = 1 << 11;

pub const SEMANTIC_SHARE_ACCOUNTING: u64 = 1 << 0;
pub const SEMANTIC_CEI_STATE_LOCK: u64 = 1 << 1;
pub const SEMANTIC_PRECISION_DIV: u64 = 1 << 2;
pub const SEMANTIC_INVARIANT_CONSERVATION: u64 = 1 << 3;
pub const SEMANTIC_CROSS_CONTRACT_VIEW: u64 = 1 << 4;
pub const SEMANTIC_SOLVENCY_BOUNDARY: u64 = 1 << 5;
pub const SEMANTIC_TEMPORAL_HORIZON: u64 = 1 << 6;
pub const SEMANTIC_GOVERNANCE_AUTH: u64 = 1 << 7;

pub const fn fnv1a_hash_bounded(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    let limit = match bytes.len() > MAX_HASH_SCAN_BYTES {
        true => MAX_HASH_SCAN_BYTES,
        false => bytes.len(),
    };
    let mut i = 0usize;
    while i < limit {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

pub const fn fnv1a_hash_ascii_lower(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    let limit = match bytes.len() > MAX_HASH_SCAN_BYTES {
        true => MAX_HASH_SCAN_BYTES,
        false => bytes.len(),
    };
    let mut i = 0usize;
    while i < limit {
        let b = bytes[i];
        let lower_b = match b >= b'A' && b <= b'Z' {
            true => b + 32,
            false => b,
        };
        hash ^= lower_b as u32;
        hash = hash.wrapping_mul(FNV_PRIME);
        i += 1;
    }
    hash
}

pub const HASH_CHOLESKY: u32 = fnv1a_hash_ascii_lower(b"cholesky");
pub const HASH_JACOBI: u32 = fnv1a_hash_ascii_lower(b"jacobi");
pub const HASH_JACOBI_EIGEN: u32 = fnv1a_hash_ascii_lower(b"jacobi_eigenvalue");
pub const HASH_QR: u32 = fnv1a_hash_ascii_lower(b"qr");
pub const HASH_GRAM_SCHMIDT: u32 = fnv1a_hash_ascii_lower(b"gram_schmidt");
pub const HASH_QR_GRAM_SCHMIDT: u32 = fnv1a_hash_ascii_lower(b"qr_gram_schmidt");
pub const HASH_DOOLITTLE: u32 = fnv1a_hash_ascii_lower(b"doolittle");
pub const HASH_DOOLITTLE_LU: u32 = fnv1a_hash_ascii_lower(b"doolittle_lu");
pub const HASH_LU: u32 = fnv1a_hash_ascii_lower(b"lu");
pub const HASH_POLAR: u32 = fnv1a_hash_ascii_lower(b"polar");
pub const HASH_HOPF: u32 = fnv1a_hash_ascii_lower(b"hopf");
pub const HASH_PYRAMID: u32 = fnv1a_hash_ascii_lower(b"pyramid");
pub const HASH_TETRAHEDRON: u32 = fnv1a_hash_ascii_lower(b"tetrahedron");
pub const HASH_OCTAHEDRON: u32 = fnv1a_hash_ascii_lower(b"octahedron");
pub const HASH_POLYGON: u32 = fnv1a_hash_ascii_lower(b"polygon");
pub const HASH_APEIROGON: u32 = fnv1a_hash_ascii_lower(b"apeirogon");
pub const HASH_NGON: u32 = fnv1a_hash_ascii_lower(b"ngon");
pub const HASH_ISOPERIMETRIC: u32 = fnv1a_hash_ascii_lower(b"isoperimetric");
pub const HASH_CARTESIAN: u32 = fnv1a_hash_ascii_lower(b"cartesian");
pub const HASH_AMM: u32 = fnv1a_hash_ascii_lower(b"amm");
pub const HASH_DEPHASING: u32 = fnv1a_hash_ascii_lower(b"dephasing");
pub const HASH_QUANTUM: u32 = fnv1a_hash_ascii_lower(b"quantum");
pub const HASH_HILBERT: u32 = fnv1a_hash_ascii_lower(b"hilbert");
pub const HASH_DENSITY: u32 = fnv1a_hash_ascii_lower(b"density");
pub const HASH_SYMPLECTIC: u32 = fnv1a_hash_ascii_lower(b"symplectic");
pub const HASH_RICCI: u32 = fnv1a_hash_ascii_lower(b"ricci");
pub const HASH_LIE: u32 = fnv1a_hash_ascii_lower(b"lie");
pub const HASH_CASIMIR: u32 = fnv1a_hash_ascii_lower(b"casimir");
pub const HASH_NOETHER: u32 = fnv1a_hash_ascii_lower(b"noether");
pub const HASH_YANG: u32 = fnv1a_hash_ascii_lower(b"yang");
pub const HASH_MILLS: u32 = fnv1a_hash_ascii_lower(b"mills");
pub const HASH_KRAMERS: u32 = fnv1a_hash_ascii_lower(b"kramers");
pub const HASH_WANNIER: u32 = fnv1a_hash_ascii_lower(b"wannier");

pub fn extract_token_hashes_stack(input: &str) -> ([u32; MAX_TOKEN_SLOTS], usize) {
    let bytes = input.as_bytes();
    let limit = match bytes.len() > MAX_HASH_SCAN_BYTES {
        true => MAX_HASH_SCAN_BYTES,
        false => bytes.len(),
    };
    let mut hashes = [0u32; MAX_TOKEN_SLOTS];
    let mut count = 0usize;
    let mut i = 0usize;

    while i < limit && count < MAX_TOKEN_SLOTS {
        while i < limit && !(bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        let start = i;
        while i < limit && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
            i += 1;
        }
        match i > start {
            true => {
                hashes[count] = fnv1a_hash_ascii_lower(&bytes[start..i]);
                count += 1;
            }
            false => {}
        }
    }
    (hashes, count)
}

pub fn has_token_hash(hashes: &[u32; MAX_TOKEN_SLOTS], count: usize, target: u32) -> bool {
    let mut i = 0usize;
    while i < count {
        match hashes[i] == target {
            true => return true,
            false => i += 1,
        }
    }
    false
}

pub fn contains_prompt_injection(input: &str) -> bool {
    let lower = input.to_lowercase();
    const PATTERNS: &[&str] = &[
        "ignore previous instructions",
        "system prompt",
        "drop table",
        "bypass security",
        "jailbreak",
        "eval(",
        "os.system",
        "subprocess",
    ];
    let mut i = 0usize;
    while i < PATTERNS.len() {
        match lower.contains(PATTERNS[i]) {
            true => return true,
            false => i += 1,
        }
    }
    false
}

pub fn verify_code_safety(code: &str) -> Result<(), &'static str> {
    match contains_prompt_injection(code) {
        true => Err("Security violation: prohibited execution pattern detected in axiom"),
        false => Ok(()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathematicalOperatorDomain {
    QuantumOperator,
    MatrixFactorization,
    SpectralIteration,
    PolyhedronGeometry,
    DynamicStepping,
    InvariantConservation,
    IterativeRelaxation,
    ProjectionGrammar,
    SymplecticLieManifold,
    Unclassified,
}

impl MathematicalOperatorDomain {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::QuantumOperator => "quantum_operator",
            Self::MatrixFactorization => "matrix_factorization",
            Self::SpectralIteration => "spectral_iteration",
            Self::PolyhedronGeometry => "polyhedron_geometry",
            Self::DynamicStepping => "dynamic_stepping",
            Self::InvariantConservation => "invariant_conservation",
            Self::IterativeRelaxation => "iterative_relaxation",
            Self::ProjectionGrammar => "projection_grammar",
            Self::SymplecticLieManifold => "symplectic_lie_manifold",
            Self::Unclassified => "unclassified",
        }
    }

    pub fn from_category_str(s: &str) -> Self {
        let low = s.trim().to_lowercase();
        match low.as_str() {
            "quantum_operator" => Self::QuantumOperator,
            "matrix_factorization" => Self::MatrixFactorization,
            "spectral_iteration" => Self::SpectralIteration,
            "polyhedron_geometry" | "spatial_geometry" => Self::PolyhedronGeometry,
            "dynamic_stepping" | "numerical_stepping" => Self::DynamicStepping,
            "invariant_conservation" | "economic_invariant" => Self::InvariantConservation,
            "iterative_relaxation" => Self::IterativeRelaxation,
            "projection_grammar" | "subspace_projection" => Self::ProjectionGrammar,
            "symplectic_lie_manifold" => Self::SymplecticLieManifold,
            _ => Self::classify(s),
        }
    }

    pub fn classify(topic: &str) -> Self {
        let t = topic.to_lowercase();
        const PROFILES: &[(MathematicalOperatorDomain, &[&str])] = &[
            (
                MathematicalOperatorDomain::SymplecticLieManifold,
                &[
                    "symplectic", "lie", "casimir", "ricci", "yang", "mills", "noether",
                    "kramers", "wannier", "hamiltonian", "poisson", "chern", "simons",
                    "calabi", "yau", "kahler", "finsler", "cartan", "killing",
                ],
            ),
            (
                MathematicalOperatorDomain::QuantumOperator,
                &[
                    "quantum", "qubit", "density", "trace", "lindblad", "entropy",
                    "neumann", "bell", "pauli", "decoherence", "hilbert", "entanglement",
                    "ghz", "dephasing", "damping", "kraus", "bloch", "unitary",
                ],
            ),
            (
                MathematicalOperatorDomain::MatrixFactorization,
                &[
                    "crout", "cholesky", "doolittle", "factorization", "decomposition",
                    "ldlt", "householder", "givens", "tridiagonal", "svd", "schur",
                    "lu", "qr", "polar", "sylvester", "lyapunov", "cayley",
                ],
            ),
            (
                MathematicalOperatorDomain::SpectralIteration,
                &[
                    "arnoldi", "krylov", "power", "rayleigh", "eigen", "spectral",
                    "gmres", "bicgstab", "hessenberg", "jacobi", "lanczos", "gershgorin",
                    "perron", "frobenius", "cheeger", "laplacian",
                ],
            ),
            (
                MathematicalOperatorDomain::PolyhedronGeometry,
                &[
                    "tesseract", "hypercube", "pyramid", "tetrahedron", "octahedron",
                    "polyhedron", "polytope", "polygon", "apeirogon", "ngon", "isoperimetric",
                    "archimedes", "exhaustion", "cartesian", "abscissa", "ordinate",
                    "manifold", "topology", "simplex", "icosahedron", "dodecahedron",
                    "euler", "betti", "quaternion", "hopf", "fibration", "gauss_bonnet",
                ],
            ),
            (
                MathematicalOperatorDomain::DynamicStepping,
                &[
                    "stepping", "runge", "kutta", "verlet", "trajectory", "integrator",
                    "oscillator", "transport", "navier", "ode", "pde", "wave", "heat",
                    "kuramoto", "fokker", "planck", "ito", "stratonovich",
                ],
            ),
            (
                MathematicalOperatorDomain::InvariantConservation,
                &[
                    "invariant", "conservation", "preservation", "accounting", "vault",
                    "share", "pool", "amm", "solvency", "cumulative_delta", "muldiv",
                ],
            ),
            (
                MathematicalOperatorDomain::IterativeRelaxation,
                &["conjugate", "gradient", "relaxation", "seidel", "sor", "richardson"],
            ),
            (
                MathematicalOperatorDomain::ProjectionGrammar,
                &["projection", "subspace", "orthogonal", "idempotent", "gram"],
            ),
        ];

        let mut best_domain = MathematicalOperatorDomain::Unclassified;
        let mut best_score = 0usize;
        let mut i = 0usize;
        while i < PROFILES.len() {
            let (domain, kws) = PROFILES[i];
            let mut score = 0usize;
            let mut j = 0usize;
            while j < kws.len() {
                match t.contains(kws[j]) {
                    true => score += 1,
                    false => {}
                }
                j += 1;
            }
            match score > best_score {
                true => {
                    best_score = score;
                    best_domain = domain;
                }
                false => {}
            }
            i += 1;
        }
        best_domain
    }

    pub const fn semantic_bitmask(&self) -> u64 {
        match self {
            Self::QuantumOperator => {
                MASK_QUANTUM | SEMANTIC_INVARIANT_CONSERVATION | SEMANTIC_TEMPORAL_HORIZON
            }
            Self::MatrixFactorization => {
                MASK_FACTORIZATION | SEMANTIC_PRECISION_DIV | SEMANTIC_INVARIANT_CONSERVATION
            }
            Self::SpectralIteration => {
                MASK_SPECTRAL | SEMANTIC_INVARIANT_CONSERVATION | SEMANTIC_TEMPORAL_HORIZON
            }
            Self::PolyhedronGeometry => MASK_POLYHEDRON | SEMANTIC_INVARIANT_CONSERVATION,
            Self::DynamicStepping => {
                MASK_DYNAMIC_STEP | SEMANTIC_TEMPORAL_HORIZON | SEMANTIC_INVARIANT_CONSERVATION
            }
            Self::InvariantConservation => {
                MASK_ECONOMIC_AMM
                    | MASK_AUDIT_ARITHMETIC
                    | SEMANTIC_SHARE_ACCOUNTING
                    | SEMANTIC_INVARIANT_CONSERVATION
                    | SEMANTIC_SOLVENCY_BOUNDARY
            }
            Self::IterativeRelaxation => MASK_SPECTRAL | SEMANTIC_PRECISION_DIV,
            Self::ProjectionGrammar => MASK_FACTORIZATION | SEMANTIC_INVARIANT_CONSERVATION,
            Self::SymplecticLieManifold => {
                MASK_SYMPLECTIC_LIE
                    | MASK_RICCI_MANIFOLD
                    | MASK_SPECTRAL
                    | SEMANTIC_INVARIANT_CONSERVATION
                    | SEMANTIC_TEMPORAL_HORIZON
            }
            Self::Unclassified => SEMANTIC_INVARIANT_CONSERVATION,
        }
    }
}

pub fn compute_problem_bitmask(input: &str) -> u64 {
    let (hashes, count) = extract_token_hashes_stack(input);
    let mut mask = 0u64;
    let mut i = 0usize;

    while i < count {
        match hashes[i] {
            HASH_QUANTUM | HASH_HILBERT | HASH_DENSITY => mask |= MASK_QUANTUM,
            HASH_DEPHASING => mask |= MASK_QUANTUM | MASK_DEPHASING,
            HASH_CHOLESKY | HASH_DOOLITTLE | HASH_DOOLITTLE_LU | HASH_LU | HASH_QR
            | HASH_GRAM_SCHMIDT | HASH_QR_GRAM_SCHMIDT | HASH_POLAR => mask |= MASK_FACTORIZATION,
            HASH_JACOBI | HASH_JACOBI_EIGEN => mask |= MASK_SPECTRAL,
            HASH_PYRAMID | HASH_TETRAHEDRON | HASH_OCTAHEDRON | HASH_HOPF | HASH_POLYGON
            | HASH_APEIROGON | HASH_NGON | HASH_ISOPERIMETRIC | HASH_CARTESIAN => {
                mask |= MASK_POLYHEDRON
            }
            HASH_AMM => mask |= MASK_ECONOMIC_AMM | MASK_AUDIT_ARITHMETIC,
            HASH_SYMPLECTIC | HASH_LIE | HASH_CASIMIR | HASH_NOETHER => {
                mask |= MASK_SYMPLECTIC_LIE | MASK_SPECTRAL
            }
            HASH_RICCI | HASH_YANG | HASH_MILLS | HASH_KRAMERS | HASH_WANNIER => {
                mask |= MASK_RICCI_MANIFOLD | MASK_SYMPLECTIC_LIE | MASK_POLYHEDRON
            }
            _ => {}
        }
        i += 1;
    }

    match mask == 0 {
        true => MathematicalOperatorDomain::classify(input).semantic_bitmask(),
        false => mask,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SynthesizedAxiomRecord {
    pub axiom_id: u32,
    pub key: String,
    pub title: String,
    pub domain: MathematicalOperatorDomain,
    pub principle: String,
    pub invariant_formula: String,
    pub semantic_mask: u64,
}

fn escape_json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            c => out.push(c),
        }
    }
    out
}

fn extract_json_field<'a>(json_line: &'a str, field_name: &str) -> Option<&'a str> {
    let pattern = format!("\"{}\":\"", field_name);
    match json_line.find(&pattern) {
        Some(start_pos) => {
            let content_start = start_pos + pattern.len();
            let rest = &json_line[content_start..];
            let bytes = rest.as_bytes();
            let mut idx = 0usize;
            while idx < bytes.len() {
                match bytes[idx] {
                    b'\\' => idx += 2,
                    b'"' => return Some(&rest[..idx]),
                    _ => idx += 1,
                }
            }
            None
        }
        None => None,
    }
}

impl SynthesizedAxiomRecord {
    pub fn to_jsonl_line(&self) -> String {
        format!(
            "{{\"key\":\"{}\",\"title\":\"{}\",\"category\":\"{}\",\"principle\":\"{}\",\"diagonal_formula\":\"{}\",\"off_diagonal_formula\":\"0.0\",\"output_symbol\":\"invariant_val\",\"pre_conditions\":\"True\",\"core_algorithm\":\"{}\",\"post_conditions\":\"True\"}}",
            escape_json_str(&self.key),
            escape_json_str(&self.title),
            self.domain.as_str(),
            escape_json_str(&self.principle),
            escape_json_str(&self.invariant_formula),
            escape_json_str(&self.invariant_formula),
        )
    }

    pub fn from_jsonl_line(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        match trimmed.starts_with('{') && trimmed.ends_with('}') {
            false => return None,
            true => {}
        }
        let key = extract_json_field(trimmed, "key")?;
        let title = extract_json_field(trimmed, "title").unwrap_or(key);
        let category = extract_json_field(trimmed, "category").unwrap_or("invariant_conservation");
        let principle = extract_json_field(trimmed, "principle")
            .unwrap_or("Conservation of mathematical structure under canonical transformation.");
        let formula = extract_json_field(trimmed, "diagonal_formula")
            .or_else(|| extract_json_field(trimmed, "core_algorithm"))
            .unwrap_or("Tr(A) == Tr(A)");

        let domain = MathematicalOperatorDomain::from_category_str(category);
        let sem = domain.semantic_bitmask() | compute_problem_bitmask(key);
        let axiom_id = (fnv1a_hash_ascii_lower(key.as_bytes()) % 900) + 100;

        Some(Self {
            axiom_id,
            key: key.to_string(),
            title: title.to_string(),
            domain,
            principle: principle.to_string(),
            invariant_formula: formula.to_string(),
            semantic_mask: sem,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AxiomFastGateEntry {
    pub signature_hash: u32,
    pub trigger_mask: u64,
    pub semantic_bitmask: u64,
    pub axiom_id: u32,
}

impl AxiomFastGateEntry {
    pub const fn new(
        signature_hash: u32,
        trigger_mask: u64,
        semantic_bitmask: u64,
        axiom_id: u32,
    ) -> Self {
        Self {
            signature_hash,
            trigger_mask,
            semantic_bitmask,
            axiom_id,
        }
    }

    pub const fn resonance_weight_with(&self, other: &Self) -> f64 {
        let shared_semantics = (self.semantic_bitmask & other.semantic_bitmask).count_ones();
        let union_semantics = (self.semantic_bitmask | other.semantic_bitmask).count_ones();
        let shared_triggers = (self.trigger_mask & other.trigger_mask).count_ones();

        let denom = match union_semantics == 0 {
            true => 1u32,
            false => union_semantics,
        };
        let base_ratio = (shared_semantics as f64) / (denom as f64);
        let trigger_bonus = (shared_triggers as f64) * 0.125;
        base_ratio + trigger_bonus
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcceptDenyFunnel {
    pub accepted: [Option<AxiomFastGateEntry>; MAX_ACCEPT_SLOTS],
    pub accepted_count: usize,
    pub denied_count: usize,
}

impl Default for AcceptDenyFunnel {
    fn default() -> Self {
        Self::new()
    }
}

impl AcceptDenyFunnel {
    pub const fn new() -> Self {
        Self {
            accepted: [None; MAX_ACCEPT_SLOTS],
            accepted_count: 0,
            denied_count: 0,
        }
    }

    pub fn evaluate(&mut self, problem_mask: u64, candidate: AxiomFastGateEntry) {
        let trigger_matched =
            (problem_mask & candidate.trigger_mask) != 0 && candidate.trigger_mask != 0;
        let semantic_resonated =
            (problem_mask & candidate.semantic_bitmask) != 0 && candidate.semantic_bitmask != 0;
        match (trigger_matched || semantic_resonated) && self.accepted_count < MAX_ACCEPT_SLOTS {
            true => {
                self.accepted[self.accepted_count] = Some(candidate);
                self.accepted_count += 1;
            }
            false => {
                self.denied_count = self.denied_count.saturating_add(1);
            }
        }
    }
}

pub const BUILTIN_FAST_GATE_CATALOG: [AxiomFastGateEntry; CATALOG_GATE_SIZE] = [
    AxiomFastGateEntry::new(
        HASH_CHOLESKY,
        MASK_FACTORIZATION,
        SEMANTIC_PRECISION_DIV | SEMANTIC_INVARIANT_CONSERVATION,
        101,
    ),
    AxiomFastGateEntry::new(
        HASH_DOOLITTLE_LU,
        MASK_FACTORIZATION,
        SEMANTIC_PRECISION_DIV | SEMANTIC_INVARIANT_CONSERVATION,
        102,
    ),
    AxiomFastGateEntry::new(
        HASH_QR_GRAM_SCHMIDT,
        MASK_FACTORIZATION | MASK_SPECTRAL,
        SEMANTIC_PRECISION_DIV | SEMANTIC_INVARIANT_CONSERVATION,
        103,
    ),
    AxiomFastGateEntry::new(
        HASH_JACOBI_EIGEN,
        MASK_SPECTRAL,
        SEMANTIC_INVARIANT_CONSERVATION | SEMANTIC_TEMPORAL_HORIZON,
        201,
    ),
    AxiomFastGateEntry::new(
        HASH_PYRAMID,
        MASK_POLYHEDRON,
        SEMANTIC_INVARIANT_CONSERVATION,
        301,
    ),
    AxiomFastGateEntry::new(
        HASH_TETRAHEDRON,
        MASK_POLYHEDRON,
        SEMANTIC_INVARIANT_CONSERVATION,
        302,
    ),
    AxiomFastGateEntry::new(
        HASH_OCTAHEDRON,
        MASK_POLYHEDRON,
        SEMANTIC_INVARIANT_CONSERVATION,
        303,
    ),
    AxiomFastGateEntry::new(
        HASH_AMM,
        MASK_ECONOMIC_AMM | MASK_AUDIT_ARITHMETIC,
        SEMANTIC_SHARE_ACCOUNTING
            | SEMANTIC_PRECISION_DIV
            | SEMANTIC_INVARIANT_CONSERVATION
            | SEMANTIC_SOLVENCY_BOUNDARY,
        401,
    ),
    AxiomFastGateEntry::new(
        HASH_DEPHASING,
        MASK_QUANTUM | MASK_DEPHASING,
        SEMANTIC_INVARIANT_CONSERVATION | SEMANTIC_TEMPORAL_HORIZON,
        501,
    ),
];

pub fn filter_axioms_through_funnel(
    problem_mask: u64,
    custom_candidates: &[AxiomFastGateEntry],
) -> AcceptDenyFunnel {
    let mut funnel = AcceptDenyFunnel::new();
    let custom_limit = match custom_candidates.len() > 64 {
        true => 64,
        false => custom_candidates.len(),
    };
    let mut c_idx = 0usize;
    while c_idx < custom_limit {
        funnel.evaluate(problem_mask, custom_candidates[c_idx]);
        c_idx += 1;
    }

    let mut b_idx = 0usize;
    while b_idx < CATALOG_GATE_SIZE {
        funnel.evaluate(problem_mask, BUILTIN_FAST_GATE_CATALOG[b_idx]);
        b_idx += 1;
    }
    funnel
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipelineExecutionReport {
    pub converged: bool,
    pub slides_executed: usize,
    pub stages_verified: usize,
    pub broken_axiom_id: Option<u32>,
    pub global_invariants: [f64; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SlidingAxiomPipeline {
    pub window: [Option<AxiomFastGateEntry>; MAX_PIPELINE_WINDOW],
    pub scratchpad: [f64; SCRATCHPAD_SIZE],
    pub attention_matrix_4x4: [[f64; 4]; 4],
    pub current_offset: usize,
    pub slides_count: usize,
    pub halted_fail_closed: bool,
    pub broken_axiom_id: Option<u32>,
}

impl Default for SlidingAxiomPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl SlidingAxiomPipeline {
    pub const fn new() -> Self {
        Self {
            window: [None; MAX_PIPELINE_WINDOW],
            scratchpad: [0.0; SCRATCHPAD_SIZE],
            attention_matrix_4x4: [[0.0; 4]; 4],
            current_offset: 0,
            slides_count: 0,
            halted_fail_closed: false,
            broken_axiom_id: None,
        }
    }

    pub fn load_window_at(&mut self, funnel: &AcceptDenyFunnel, offset: usize) {
        self.window = [None; MAX_PIPELINE_WINDOW];
        self.current_offset = offset;

        let mut s_idx = 4usize;
        while s_idx < SCRATCHPAD_SIZE {
            self.scratchpad[s_idx] = 0.0;
            s_idx += 1;
        }

        let mut w_idx = 0usize;
        while w_idx < MAX_PIPELINE_WINDOW {
            let src_idx = offset + w_idx;
            match src_idx < funnel.accepted_count {
                true => self.window[w_idx] = funnel.accepted[src_idx],
                false => match funnel.accepted_count > 0 {
                    true => {
                        let wrapped = src_idx % funnel.accepted_count;
                        self.window[w_idx] = funnel.accepted[wrapped];
                    }
                    false => {}
                },
            }
            w_idx += 1;
        }

        let mut r = 0usize;
        while r < 4 {
            let mut c = 0usize;
            while c < 4 {
                self.attention_matrix_4x4[r][c] = match (self.window[r], self.window[c]) {
                    (Some(a), Some(b)) => a.resonance_weight_with(&b),
                    _ => 0.0,
                };
                c += 1;
            }
            r += 1;
        }
    }

    pub fn execute_sliding_chain<F>(
        &mut self,
        funnel: &AcceptDenyFunnel,
        mut step_evaluator: F,
    ) -> PipelineExecutionReport
    where
        F: FnMut(AxiomFastGateEntry, usize, &mut [f64; SCRATCHPAD_SIZE]) -> bool,
    {
        match funnel.accepted_count == 0 {
            true => {
                self.halted_fail_closed = true;
                return PipelineExecutionReport {
                    converged: false,
                    slides_executed: 0,
                    stages_verified: 0,
                    broken_axiom_id: None,
                    global_invariants: [0.0; 4],
                };
            }
            false => {}
        }

        let raw_frames = match funnel.accepted_count > MAX_PIPELINE_WINDOW {
            true => (funnel.accepted_count - MAX_PIPELINE_WINDOW) + 1,
            false => 1,
        };
        let total_frames = match raw_frames > MAX_SLIDE_BUDGET {
            true => MAX_SLIDE_BUDGET,
            false => raw_frames,
        };

        let mut frame_idx = 0usize;
        let mut stages_verified = 0usize;

        while frame_idx < total_frames {
            self.load_window_at(funnel, frame_idx);
            self.slides_count += 1;

            let mut slot_idx = 0usize;
            while slot_idx < MAX_PIPELINE_WINDOW {
                match self.window[slot_idx] {
                    Some(entry) => {
                        let passed = step_evaluator(entry, slot_idx, &mut self.scratchpad);
                        let resonance_boost = self.attention_matrix_4x4[0][slot_idx];
                        let local_slot = 4 + (slot_idx * 3) + 2;
                        match local_slot < SCRATCHPAD_SIZE {
                            true => self.scratchpad[local_slot] += resonance_boost,
                            false => {}
                        }
                        match passed {
                            true => stages_verified += 1,
                            false => {
                                self.halted_fail_closed = true;
                                self.broken_axiom_id = Some(entry.axiom_id);
                                return PipelineExecutionReport {
                                    converged: false,
                                    slides_executed: self.slides_count,
                                    stages_verified,
                                    broken_axiom_id: Some(entry.axiom_id),
                                    global_invariants: [
                                        self.scratchpad[0],
                                        self.scratchpad[1],
                                        self.scratchpad[2],
                                        self.scratchpad[3],
                                    ],
                                };
                            }
                        }
                    }
                    None => {}
                }
                slot_idx += 1;
            }
            frame_idx += 1;
        }

        PipelineExecutionReport {
            converged: true,
            slides_executed: self.slides_count,
            stages_verified,
            broken_axiom_id: None,
            global_invariants: [
                self.scratchpad[0],
                self.scratchpad[1],
                self.scratchpad[2],
                self.scratchpad[3],
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwarmBatchSummary {
    pub committed_count: usize,
    pub rejected_count: usize,
    pub active_l2_nodes: usize,
    pub active_hyperedges: usize,
    pub terminal_seal_crc32: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SwarmResearchCoordinator;

impl SwarmResearchCoordinator {
    pub const fn new() -> Self {
        Self
    }

    pub fn parse_axiom_spec(raw_spec: &str) -> Option<SynthesizedAxiomRecord> {
        let trimmed = raw_spec
            .trim()
            .trim_start_matches('-')
            .trim_start_matches('*')
            .trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')
            .trim();

        match trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with("//") {
            true => return None,
            false => {}
        }

        match contains_prompt_injection(trimmed) {
            true => return None,
            false => {}
        }

        match trimmed.contains('|') && !trimmed.contains(":=") {
            true => {
                let mut parts: [&str; 4] = [""; 4];
                let mut p_cnt = 0usize;
                for seg in trimmed.split('|') {
                    let clean_seg = seg.trim().trim_matches('`').trim_matches('*').trim();
                    match !clean_seg.is_empty() && p_cnt < 4 {
                        true => {
                            parts[p_cnt] = clean_seg;
                            p_cnt += 1;
                        }
                        false => {}
                    }
                }
                match p_cnt >= 2 {
                    true => {
                        let raw_key = parts[0];
                        let clean_key = sanitize_identifier_key(raw_key);
                        match clean_key.is_empty() {
                            true => return None,
                            false => {}
                        }
                        let (title, principle, formula) = match p_cnt {
                            2 => (
                                format!("Formal Conservation Axiom ({})", clean_key),
                                format!(
                                    "Universal invariance law governing {} under canonical group action.",
                                    clean_key
                                ),
                                parts[1].to_string(),
                            ),
                            3 => (
                                format!("Formal Conservation Axiom ({})", clean_key),
                                parts[2].to_string(),
                                parts[1].to_string(),
                            ),
                            _ => (
                                parts[1].to_string(),
                                parts[2].to_string(),
                                parts[3].to_string(),
                            ),
                        };
                        let domain = MathematicalOperatorDomain::classify(&format!(
                            "{} {} {}",
                            clean_key, title, principle
                        ));
                        let sem = domain.semantic_bitmask() | compute_problem_bitmask(&clean_key);
                        return Some(SynthesizedAxiomRecord {
                            axiom_id: 0,
                            key: clean_key,
                            title,
                            domain,
                            principle,
                            invariant_formula: formula,
                            semantic_mask: sem,
                        });
                    }
                    false => {}
                }
            }
            false => {}
        }

        let sep_opt = trimmed
            .find(":=")
            .map(|i| (i, 2usize))
            .or_else(|| trimmed.find(':').map(|i| (i, 1usize)));

        match sep_opt {
            Some((sep_idx, sep_len)) => {
                let raw_left = trimmed[..sep_idx]
                    .trim()
                    .trim_matches('*')
                    .trim_matches('`')
                    .trim();
                let raw_right = trimmed[sep_idx + sep_len..].trim();
                let clean_key = sanitize_identifier_key(raw_left);
                match !clean_key.is_empty() && !raw_right.is_empty() {
                    true => {
                        let (formula_part, principle_part) = match raw_right.find('|') {
                            Some(p_idx) => (
                                raw_right[..p_idx].trim().trim_matches('`').trim(),
                                raw_right[p_idx + 1..].trim(),
                            ),
                            None => (
                                raw_right.trim_matches('`').trim(),
                                "Exact conservation of algebraic and topological invariants across state transitions.",
                            ),
                        };
                        let domain = MathematicalOperatorDomain::classify(&format!(
                            "{} {} {}",
                            clean_key, formula_part, principle_part
                        ));
                        let title = format!("Formal {} Invariant ({})", domain.as_str(), clean_key);
                        let sem = domain.semantic_bitmask() | compute_problem_bitmask(&clean_key);
                        return Some(SynthesizedAxiomRecord {
                            axiom_id: 0,
                            key: clean_key,
                            title,
                            domain,
                            principle: principle_part.to_string(),
                            invariant_formula: formula_part.to_string(),
                            semantic_mask: sem,
                        });
                    }
                    false => {}
                }
            }
            None => {}
        }

        let clean_key = sanitize_identifier_key(trimmed);
        match clean_key.is_empty() {
            true => None,
            false => {
                let domain = MathematicalOperatorDomain::classify(&clean_key);
                let (title, principle, formula) = match domain {
                    MathematicalOperatorDomain::QuantumOperator => (
                        format!("Quantum Operator & Trace Preservation ({})", clean_key),
                        "Unitary & Kraus quantum channels preserve trace normalization Tr(rho) == 1.0 and purity bounds.".to_string(),
                        "Tr(A) + purity".to_string(),
                    ),
                    MathematicalOperatorDomain::MatrixFactorization => (
                        format!("Canonical Matrix Factorization Invariant ({})", clean_key),
                        "Exact triangular and unitary factorization preserves determinant and Frobenius norm.".to_string(),
                        "det(A) + norm(A)".to_string(),
                    ),
                    MathematicalOperatorDomain::SpectralIteration => (
                        format!("Spectral Eigenvalue Conservation ({})", clean_key),
                        "Orthogonal similarity transformations preserve the spectral trace and characteristic polynomial.".to_string(),
                        "Tr(A) + eig1 + eig2".to_string(),
                    ),
                    MathematicalOperatorDomain::PolyhedronGeometry => (
                        format!("Polygonal, Apeirogonal & Topological Invariant ({})", clean_key),
                        "Archimedes exhaustion and homeomorphic transformations preserve Euler characteristic and isoperimetric bounds.".to_string(),
                        "pi * r^2 + V - E + F".to_string(),
                    ),
                    MathematicalOperatorDomain::DynamicStepping => (
                        format!("Symplectic Phase-Space Integrator ({})", clean_key),
                        "Symplectic time-stepping preserves phase-space volume and Hamiltonian energy bounds.".to_string(),
                        "Tr(A) * exp(-lambda * dt)".to_string(),
                    ),
                    MathematicalOperatorDomain::SymplecticLieManifold => (
                        format!("Symplectic Lie-Ricci Casimir Conservation ({})", clean_key),
                        "Non-Abelian Lie-Poisson brackets and Ricci-Hamiltonian flows conserve Casimir invariants and Kramers-Wannier duality.".to_string(),
                        "alpha * Tr(A) + beta * det(A) + gamma * chi".to_string(),
                    ),
                    _ => (
                        format!("Universal Conservation Law ({})", clean_key),
                        "Total state invariant is strictly conserved across deterministic transformations.".to_string(),
                        "Tr(A) + det(A)".to_string(),
                    ),
                };
                let sem = domain.semantic_bitmask() | compute_problem_bitmask(&clean_key);
                Some(SynthesizedAxiomRecord {
                    axiom_id: 0,
                    key: clean_key,
                    title,
                    domain,
                    principle,
                    invariant_formula: formula,
                    semantic_mask: sem,
                })
            }
        }
    }

    pub fn persist_axiom_to_knowledge_jsonl(&self, record: &SynthesizedAxiomRecord) {
        let dir = Path::new("garry_workspace").join("knowledge");
        let _ = std::fs::create_dir_all(&dir);
        let file_path = dir.join(format!("{}.jsonl", record.key));
        let line = record.to_jsonl_line();
        match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&file_path)
        {
            Ok(mut f) => {
                let _ = writeln!(f, "{}", line);
                let _ = f.sync_all();
            }
            Err(_) => {}
        }
    }

    pub fn load_persisted_knowledge_dir(&self, l2_hypergraph: &mut AxiomHyperGraphL2) -> usize {
        let dir = Path::new("garry_workspace").join("knowledge");
        match dir.is_dir() {
            false => return 0,
            true => {}
        }
        let mut loaded_count = 0usize;
        match std::fs::read_dir(&dir) {
            Ok(entries) => {
                let mut budget = 256usize;
                for entry_res in entries {
                    match budget == 0 {
                        true => break,
                        false => budget -= 1,
                    }
                    match entry_res {
                        Ok(entry) => {
                            let path = entry.path();
                            let is_jsonl = path.is_file()
                                && match path.extension().and_then(|e| e.to_str()) {
                                    Some(ext) => ext == "jsonl",
                                    None => false,
                                };
                            match is_jsonl {
                                true => match std::fs::read_to_string(&path) {
                                    Ok(content) => {
                                        let first_line = content.lines().next().unwrap_or("").trim();
                                        match SynthesizedAxiomRecord::from_jsonl_line(first_line) {
                                            Some(rec) => {
                                                let assigned_id = l2_hypergraph.upsert_axiom(
                                                    rec.key.as_bytes(),
                                                    rec.invariant_formula.as_bytes(),
                                                    rec.semantic_mask,
                                                );
                                                match assigned_id > 0 {
                                                    true => loaded_count += 1,
                                                    false => {}
                                                }
                                            }
                                            None => {}
                                        }
                                    }
                                    Err(_) => {}
                                },
                                false => {}
                            }
                        }
                        Err(_) => {}
                    }
                }
            }
            Err(_) => {}
        }
        loaded_count
    }

    pub fn synthesize_and_commit_axiom(
        &self,
        topic: &str,
        engine: &mut DeterministicEngine,
        l2_hypergraph: &mut AxiomHyperGraphL2,
    ) -> Result<(SynthesizedAxiomRecord, u32), &'static str> {
        let mut record = Self::parse_axiom_spec(topic)
            .ok_or("Swarm rejected: empty or prohibited axiom specification")?;

        verify_code_safety(&record.invariant_formula)?;

        let mut prover = DeterministicSmtProver::new();
        let denom_hash = fnv1a_hash_ascii_lower(record.key.as_bytes());
        let _ = prover.add_precondition(SmtConstraint {
            lhs_var_hash: denom_hash,
            op: SmtRelationOp::GreaterOrEqual,
            rhs_constant: 1,
            rhs_var_hash: None,
        });
        let adversary = MathematicalAdversaryEngine::new();
        match adversary.challenge_theorem(true, true, Some(denom_hash), 0, &mut prover) {
            RefutationVerdict::SurvivedAdversary { .. } => {}
            RefutationVerdict::RefutedByAdversary { reason, .. } => return Err(reason),
        }

        let assigned_id = l2_hypergraph.upsert_axiom(
            record.key.as_bytes(),
            record.invariant_formula.as_bytes(),
            record.semantic_mask,
        );
        record.axiom_id = assigned_id;

        match engine.storage().len() >= 15 {
            true => {
                engine.storage_mut().slots[14] = None;
                engine.storage_mut().slots[15] = None;
                engine.storage_mut().count = engine.storage_mut().count.saturating_sub(2);
            }
            false => {}
        }

        match engine.state() {
            EngineState::Idle | EngineState::Verifying => {
                let _ = engine.trigger_planning();
            }
            EngineState::Completed | EngineState::Failed => {
                *engine = DeterministicEngine::new(100_000);
                let _ = engine.register_action("STORE_AXIOM", default_store_axiom_slot);
                let _ = engine.trigger_planning();
            }
            _ => {}
        }

        let k_len = record.key.len().min(MAX_KEY_SIZE);
        let f_len = record.invariant_formula.len().min(MAX_VALUE_SIZE);
        let _ = engine.storage_mut().set(
            &record.key.as_bytes()[..k_len],
            &record.invariant_formula.as_bytes()[..f_len],
        );

        match ActionProposal::new("STORE_AXIOM", &record.invariant_formula.as_bytes()[..f_len])
            .and_then(|p| p.validate(1))
        {
            Ok(prop) => {
                let _ = engine.execute_proposal(prop);
                let _ = engine.finalize_verification(true);
            }
            Err(_) => {}
        }

        self.persist_axiom_to_knowledge_jsonl(&record);

        let cp = engine.export_checkpoint_array();
        let seal = calculate_crc32(&cp[..cp.len() - 4]);
        Ok((record, seal))
    }

    pub fn ingest_markdown_swarm_batch(
        &self,
        markdown_text: &str,
        engine: &mut DeterministicEngine,
        l2_hypergraph: &mut AxiomHyperGraphL2,
    ) -> Result<SwarmBatchSummary, &'static str> {
        let mut committed = 0usize;
        let mut rejected = 0usize;
        let mut last_seal = 0u32;

        let mut segment_budget = 512usize;
        for raw_line in markdown_text.split(|c: char| c == '\n' || c == ';') {
            match segment_budget == 0 {
                true => break,
                false => segment_budget -= 1,
            }
            let trimmed = raw_line.trim();
            match trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with("```")
                || trimmed.starts_with("---")
            {
                true => continue,
                false => {}
            }
            match self.synthesize_and_commit_axiom(trimmed, engine, l2_hypergraph) {
                Ok((_, seal)) => {
                    committed += 1;
                    last_seal = seal;
                }
                Err(_) => {
                    rejected += 1;
                }
            }
        }

        match committed == 0 {
            true => Err("Swarm batch contained 0 valid formal axioms"),
            false => Ok(SwarmBatchSummary {
                committed_count: committed,
                rejected_count: rejected,
                active_l2_nodes: l2_hypergraph.active_node_count(),
                active_hyperedges: l2_hypergraph.active_edge_count(),
                terminal_seal_crc32: last_seal,
            }),
        }
    }
}

fn default_store_axiom_slot(
    payload: &[u8],
    storage: &mut BoundedStorage,
) -> Result<(), EngineError> {
    let len = payload.len().min(MAX_VALUE_SIZE);
    storage.set(b"last_committed_axiom", &payload[..len])
}

pub fn sanitize_identifier_key(raw: &str) -> String {
    let mut out = String::with_capacity(32);
    let mut budget = 32usize;
    for ch in raw.trim().chars() {
        match budget == 0 {
            true => break,
            false => {}
        }
        match ch.is_ascii_alphanumeric() {
            true => {
                out.push(ch.to_ascii_lowercase());
                budget -= 1;
            }
            false => {
                match (ch == '_' || ch == '-' || ch == ' ') && !out.is_empty() && !out.ends_with('_')
                {
                    true => {
                        out.push('_');
                        budget -= 1;
                    }
                    false => {}
                }
            }
        }
    }
    out.trim_matches('_').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swarm_markdown_batch_ingests_multiple_axioms() {
        let mut engine = DeterministicEngine::new(100_000);
        let mut l2 = AxiomHyperGraphL2::new();
        let coord = SwarmResearchCoordinator::new();

        let md = "- **ax_symplectic_01**: `alpha * Tr(A) + beta * det(A)` | Symplectic Lie-Poisson trace-determinant invariance\n\
                  - **ax_ricci_02**: `Tr(A) - kappa * chi` | Ricci scalar curvature conservation on compact manifold";
        let summary = coord
            .ingest_markdown_swarm_batch(md, &mut engine, &mut l2)
            .unwrap();
        assert_eq!(summary.committed_count, 2);
        assert!(summary.active_l2_nodes >= 2);
        assert_ne!(summary.terminal_seal_crc32, 0);
    }
}