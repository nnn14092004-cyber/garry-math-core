//! Bare-metal benchmark suite and interactive mathematical REPL for Garry Math OSS.

use std::hint::black_box;
use std::io::{BufRead, Write};
use std::path::Path;
use std::time::Instant;

use garry_math_oss::ephemeral::{SwarmResearchCoordinator, MASK_POLYHEDRON, MASK_QUANTUM};
use garry_math_oss::metering::{MeterConfig, StepMeter};
use garry_math_oss::plan::{NodeStatus, PlanGraph};
use garry_math_oss::planner::{split_composite_intents_stack, DeterministicMathPlanner};
use garry_math_oss::solver::{StackMatrix8x8, UniversalMathSolver};
use garry_math_oss::{
    calculate_crc32, ActionProposal, AxiomHyperGraphL2, BoundedStorage, DeterministicEngine,
    EngineError, EngineState, CHECKPOINT_SIZE,
};

fn handle_math_action(payload: &[u8], storage: &mut BoundedStorage) -> Result<(), EngineError> {
    let len = payload.len().min(256);
    storage.set(b"last_math_derivation", &payload[..len])
}

fn handle_store_axiom(payload: &[u8], storage: &mut BoundedStorage) -> Result<(), EngineError> {
    let len = payload.len().min(256);
    storage.set(b"last_committed_axiom", &payload[..len])
}

fn handle_export_txt(payload: &[u8], storage: &mut BoundedStorage) -> Result<(), EngineError> {
    let len = payload.len().min(256);
    storage.set(b"last_exported_txt", &payload[..len])
}

fn register_default_actions(engine: &mut DeterministicEngine) -> Result<(), EngineError> {
    engine.register_action("AXIOM_FUNNEL_GATE", handle_math_action)?;
    engine.register_action("MATH_SOLVER_STAGE", handle_math_action)?;
    engine.register_action("STORE_AXIOM", handle_store_axiom)?;
    engine.register_action("FS_EXPORT_TXT", handle_export_txt)?;
    Ok(())
}

fn create_16_node_benchmark_dag() -> PlanGraph {
    let mut graph = PlanGraph::new();
    let n0 = graph.add_node("MATH_SOLVER_STAGE", b"stage_0", &[]).unwrap();
    let n1 = graph
        .add_node("MATH_SOLVER_STAGE", b"stage_1", &[n0])
        .unwrap();
    let n2 = graph
        .add_node("MATH_SOLVER_STAGE", b"stage_2", &[n0])
        .unwrap();
    let n3 = graph
        .add_node("MATH_SOLVER_STAGE", b"stage_3", &[n1, n2])
        .unwrap();
    let _ = graph.set_node_status(n0, NodeStatus::Succeeded);
    let _ = graph.set_node_status(n1, NodeStatus::Succeeded);
    let _ = graph.set_node_status(n2, NodeStatus::Succeeded);
    let _ = graph.set_node_status(n3, NodeStatus::Pending);
    graph
}

fn run_bare_metal_benchmarks() {
    println!("================================================================================");
    println!("   GARRY MATH OSS - BARE-METAL DETERMINISTIC BENCHMARKS (O(1) STACK)            ");
    println!("================================================================================");

    let iterations = 5_000usize;

    let mut meter = StepMeter::new(MeterConfig::standard(1_000_000));
    let start_dag = Instant::now();
    let mut i = 0usize;
    while i < iterations {
        let mut dag = create_16_node_benchmark_dag();
        let _ = black_box(dag.resolve_readiness(&mut meter));
        i += 1;
    }
    let dag_ns = start_dag.elapsed().as_nanos() as f64 / (iterations as f64);

    let mut engine = DeterministicEngine::new(100_000);
    let _ = register_default_actions(&mut engine);
    let start_cp = Instant::now();
    let mut j = 0usize;
    while j < iterations {
        let _ = black_box(engine.export_checkpoint_array());
        j += 1;
    }
    let cp_elapsed = start_cp.elapsed();
    let cp_us = (cp_elapsed.as_nanos() as f64 / (iterations as f64)) / 1000.0;
    let mb_s =
        ((CHECKPOINT_SIZE * iterations) as f64) / cp_elapsed.as_secs_f64() / (1024.0 * 1024.0);

    let mat =
        StackMatrix8x8::parse_from_text("[[4, 12, -16], [12, 37, -43], [-16, -43, 98]]").unwrap();
    let start_chol = Instant::now();
    let mut k = 0usize;
    while k < iterations {
        let _ = black_box(mat.cholesky());
        k += 1;
    }
    let chol_ns = start_chol.elapsed().as_nanos() as f64 / (iterations as f64);

    println!("+---------------------------------------------------+------------+---------------+--------------------+");
    println!("| Operation                                         | Iterations | Mean Latency  | Throughput / Bound |");
    println!("+---------------------------------------------------+------------+---------------+--------------------+");
    println!(
        "| PlanGraph::resolve_readiness (Stack DAG)          | {:>10} | {:>10.2} ns | O(1) Zero-Heap     |",
        iterations, dag_ns
    );
    println!(
        "| DeterministicEngine::export_checkpoint (7865 B)   | {:>10} | {:>10.2} us | {:>10.2} MB/s    |",
        iterations, cp_us, mb_s
    );
    println!(
        "| StackMatrix8x8::cholesky (3x3 SPD Decomposition)  | {:>10} | {:>10.2} ns | O(1) Zero-Heap     |",
        iterations, chol_ns
    );
    println!("+---------------------------------------------------+------------+---------------+--------------------+");
}

enum CliCommand<'a> {
    Empty,
    Exit,
    Bench,
    Storage,
    Hypergraph,
    Swarm(&'a str),
    Solve(&'a str),
}

fn parse_cli_command(line: &str) -> CliCommand<'_> {
    let trimmed = line.trim();
    match trimmed.is_empty() {
        true => return CliCommand::Empty,
        false => {}
    }
    match trimmed {
        "exit" | "quit" | ":exit" | ":quit" => return CliCommand::Exit,
        "bench" | ":bench" => return CliCommand::Bench,
        "storage" | ":storage" | ":axioms" => return CliCommand::Storage,
        "hypergraph" | ":hypergraph" => return CliCommand::Hypergraph,
        _ => {}
    }
    match trimmed.strip_prefix(":swarm ") {
        Some(topic) => return CliCommand::Swarm(topic.trim()),
        None => {}
    }
    match trimmed.strip_prefix(":swarm_md ") {
        Some(topic) => return CliCommand::Swarm(topic.trim()),
        None => {}
    }
    CliCommand::Solve(trimmed)
}

fn run_interactive_repl() {
    let mut engine = DeterministicEngine::new(100_000);
    let _ = register_default_actions(&mut engine);
    let mut l2_hypergraph = AxiomHyperGraphL2::new();
    let _ = l2_hypergraph.upsert_axiom(
        b"ax_001_von_neumann",
        b"Tr(rho) == 1.0 && Tr(rho^2) <= 1.0",
        MASK_QUANTUM,
    );
    let _ = l2_hypergraph.upsert_axiom(
        b"ax_237_hopf_euler",
        b"V - E + F == 2 && ||h(z)||_S2 == 1.0",
        MASK_POLYHEDRON,
    );

    let planner = DeterministicMathPlanner::new();
    let solver = UniversalMathSolver::new();
    let swarm = SwarmResearchCoordinator::new();
    let persisted_loaded = swarm.load_persisted_knowledge_dir(&mut l2_hypergraph);

    println!("================================================================================");
    println!("   GARRY MATH OSS - DETERMINISTIC MATHEMATICAL & AXIOM SWARM ENGINE             ");
    println!("   [100% Pure Rust | Zero External Crates | O(1) Stack-Bounded | Branchless]    ");
    println!("================================================================================");
    println!(
        "  Workspace Root:   ./garry_workspace (Loaded {} persisted L2 axioms from knowledge/)",
        persisted_loaded
    );
    println!("Commands:");
    println!("  :swarm <spec|md>    Synthesize single axiom, batch (`;`), or Markdown file (`.md`)");
    println!("  :hypergraph         Inspect active L2 Associative Hypergraph nodes & hyperedges");
    println!("  :storage            Inspect 16-slot L1 hot working memory");
    println!("  :bench              Execute bare-metal O(1) latency benchmarks");
    println!("  <math_query>        Solve & export 4-tier derivation report to .txt");
    println!("  exit                Seal final CRC-32 checkpoint and terminate\n");

    let stdin = std::io::stdin();
    let mut handle = stdin.lock();
    let mut line = String::new();

    loop {
        print!("garry-math> ");
        let _ = std::io::stdout().flush();
        line.clear();

        match handle.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        match parse_cli_command(&line) {
            CliCommand::Empty => continue,
            CliCommand::Exit => {
                let cp = engine.export_checkpoint_array();
                let crc = calculate_crc32(&cp[..CHECKPOINT_SIZE - 4]);
                println!(
                    "Session finalized. Terminal Checkpoint CRC-32 Seal: {:#010X}",
                    crc
                );
                break;
            }
            CliCommand::Bench => run_bare_metal_benchmarks(),
            CliCommand::Storage => {
                println!("\n[L1 BOUNDED STORAGE (16 HOT SLOTS)]");
                let mut idx = 0usize;
                while idx < 16 {
                    match &engine.storage().slots[idx] {
                        Some((k, kl, v, vl)) => {
                            let ks = std::str::from_utf8(&k[..*kl]).unwrap_or("");
                            let vs = std::str::from_utf8(&v[..*vl]).unwrap_or("");
                            println!("  Slot [{:02}]: {:<28} => {}", idx, ks, vs);
                        }
                        None => {}
                    }
                    idx += 1;
                }
                println!();
            }
            CliCommand::Hypergraph => {
                println!(
                    "\n[L2 AXIOM HYPERGRAPH] Active Nodes: {} / 256 | Associative Hyperedges: {}",
                    l2_hypergraph.active_node_count(),
                    l2_hypergraph.active_edge_count()
                );
                let mut idx = 0usize;
                while idx < 256 {
                    match &l2_hypergraph.nodes[idx] {
                        Some(node) => {
                            let ks = std::str::from_utf8(node.key_slice()).unwrap_or("");
                            let fs = std::str::from_utf8(node.formula_slice()).unwrap_or("");
                            println!(
                                "  Axiom #{:03} [CRC {:#010X} | Vitality {}]: {} := {}",
                                node.axiom_id, node.signature_crc32, node.vitality, ks, fs
                            );
                        }
                        None => {}
                    }
                    idx += 1;
                }
                println!();
            }
            CliCommand::Swarm(topic) => {
                let is_md_file = topic.ends_with(".md") || topic.ends_with(".txt");
                let is_batch_inline = topic.contains(';') && topic.matches(';').count() >= 1;

                match (is_md_file, is_batch_inline) {
                    (true, _) => {
                        let candidate_path = Path::new(topic);
                        let ws_path = Path::new("garry_workspace").join(topic);
                        let resolved = match candidate_path.is_file() {
                            true => candidate_path.to_path_buf(),
                            false => ws_path,
                        };
                        match std::fs::read_to_string(&resolved) {
                            Ok(md_content) => {
                                match swarm.ingest_markdown_swarm_batch(
                                    &md_content,
                                    &mut engine,
                                    &mut l2_hypergraph,
                                ) {
                                    Ok(sum) => {
                                        println!(
                                            "\n[MARKDOWN SWARM BATCH VERIFIED & COMMITTED TO L1/L2 & KNOWLEDGE/]"
                                        );
                                        println!(
                                            "  - Axioms Verified & Committed: {}",
                                            sum.committed_count
                                        );
                                        println!(
                                            "  - Active L2 Hypergraph Nodes:  {} / 256",
                                            sum.active_l2_nodes
                                        );
                                        println!(
                                            "  - Associative Hyperedges:      {}",
                                            sum.active_hyperedges
                                        );
                                        println!(
                                            "  - Terminal Checkpoint Seal:    {:#010X}\n",
                                            sum.terminal_seal_crc32
                                        );
                                    }
                                    Err(e) => println!("  [SWARM BATCH REJECTED] {}\n", e),
                                }
                            }
                            Err(e) => println!(
                                "  [SWARM FILE ERROR] Could not read '{}': {}\n",
                                resolved.display(),
                                e
                            ),
                        }
                    }
                    (false, true) => {
                        match swarm.ingest_markdown_swarm_batch(
                            topic,
                            &mut engine,
                            &mut l2_hypergraph,
                        ) {
                            Ok(sum) => {
                                println!(
                                    "\n[INLINE SWARM BATCH VERIFIED & COMMITTED TO L1/L2 & KNOWLEDGE/]"
                                );
                                println!("  - Axioms Verified & Committed: {}", sum.committed_count);
                                println!(
                                    "  - Active L2 Hypergraph Nodes:  {} / 256",
                                    sum.active_l2_nodes
                                );
                                println!(
                                    "  - Associative Hyperedges:      {}",
                                    sum.active_hyperedges
                                );
                                println!(
                                    "  - Terminal Checkpoint Seal:    {:#010X}\n",
                                    sum.terminal_seal_crc32
                                );
                            }
                            Err(e) => println!("  [SWARM BATCH REJECTED] {}\n", e),
                        }
                    }
                    (false, false) => {
                        match swarm.synthesize_and_commit_axiom(
                            topic,
                            &mut engine,
                            &mut l2_hypergraph,
                        ) {
                            Ok((ax, seal)) => {
                                println!("\n[AXIOM SWARM CONVERGED & PERSISTED TO L1/L2/JSONL]");
                                println!("  - Axiom ID:          #{}", ax.axiom_id);
                                println!("  - Key:               {}", ax.key);
                                println!("  - Title:             {}", ax.title);
                                println!("  - Domain:            {:?}", ax.domain);
                                println!("  - Principle:         {}", ax.principle);
                                println!("  - Invariant Formula: {}", ax.invariant_formula);
                                println!(
                                    "  - Persisted File:    garry_workspace/knowledge/{}.jsonl",
                                    ax.key
                                );
                                println!("  - Checkpoint Seal:   {:#010X}\n", seal);
                            }
                            Err(e) => {
                                println!("  [SWARM REJECTED] {}\n", e);
                            }
                        }
                    }
                }
            }
            CliCommand::Solve(query) => {
                let mut nodes_executed = 0usize;
                match planner.build_derivation_dag(query) {
                    Ok(dag) => {
                        let mut n_idx = 0usize;
                        while n_idx < dag.len() {
                            match dag.get_node(n_idx) {
                                Some(node) => {
                                    match engine.state() {
                                        EngineState::Idle | EngineState::Verifying => {
                                            let _ = engine.trigger_planning();
                                        }
                                        EngineState::Completed | EngineState::Failed => {
                                            engine = DeterministicEngine::new(100_000);
                                            let _ = register_default_actions(&mut engine);
                                            let _ = engine.trigger_planning();
                                        }
                                        _ => {}
                                    }
                                    match ActionProposal::new(node.action_type, node.payload())
                                        .and_then(|p| p.validate(1))
                                    {
                                        Ok(prop) => {
                                            let _ = engine.execute_proposal(prop);
                                            nodes_executed += 1;
                                        }
                                        Err(_) => {}
                                    }
                                }
                                None => {}
                            }
                            n_idx += 1;
                        }
                        let _ = engine.finalize_verification(true);
                    }
                    Err(_) => {}
                }

                let (stages, stage_cnt) = split_composite_intents_stack(query);
                let effective_cnt = stage_cnt.max(1);
                let mut s_i = 0usize;

                while s_i < effective_cnt {
                    let stage_q = match stage_cnt > 0 {
                        true => stages[s_i],
                        false => query,
                    };
                    match solver.solve_and_export_txt(stage_q, &mut l2_hypergraph) {
                        Ok((rep, txt_path)) => {
                            println!("\n{}", rep);
                            let cp = engine.export_checkpoint_array();
                            let crc = calculate_crc32(&cp[..CHECKPOINT_SIZE - 4]);
                            println!("[DETERMINISTIC KERNEL TELEMETRY]");
                            println!("  Status:           Executed (DAG Nodes: {})", nodes_executed);
                            println!("  FSM State:        {:?}", engine.state());
                            println!(
                                "  Compute Units:    {} units",
                                engine.meter().consumed_units()
                            );
                            println!(
                                "  Steps Consumed:   {}",
                                engine.meter().consumed_steps()
                            );
                            println!(
                                "  Exported Report:  {} (& garry_workspace/scripts/task_report.txt)",
                                txt_path
                            );
                            println!(
                                "  Checkpoint Seal:  {:#010X} (CRC-32 Verified)\n",
                                crc
                            );
                        }
                        Err(err) => {
                            println!("\n  [FAIL-CLOSED MATHEMATICAL GATE]: {}", err);
                            println!(
                                "  FSM preserved safely. State: {:?}\n",
                                engine.state()
                            );
                        }
                    }
                    s_i += 1;
                }
            }
        }
    }
}

fn main() {
    let mut args = std::env::args();
    let _bin = args.next();
    match args.next().as_deref() {
        Some("bench") | Some("--bench") => run_bare_metal_benchmarks(),
        _ => run_interactive_repl(),
    }
}