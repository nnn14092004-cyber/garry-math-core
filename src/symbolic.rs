//! Stack-allocated symbolic expression AST arena, canonical algebraic simplifier,
//! formal partial differentiator, and bounded numerical evaluator.

pub const MAX_AST_NODES: usize = 64;
pub const MAX_TREE_DEPTH: usize = 32;
pub const MAX_ENV_VARS: usize = 16;
const EPSILON: f64 = 1e-12;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolicOp {
    Const(f64),
    Var(u32),
    Neg(usize),
    Sqrt(usize),
    Add(usize, usize),
    Sub(usize, usize),
    Mul(usize, usize),
    Div(usize, usize),
    Pow(usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VarEnv {
    pub bindings: [Option<(u32, f64)>; MAX_ENV_VARS],
    pub count: usize,
}

impl Default for VarEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl VarEnv {
    pub const fn new() -> Self {
        Self {
            bindings: [None; MAX_ENV_VARS],
            count: 0,
        }
    }

    pub fn bind(&mut self, var_hash: u32, value: f64) -> bool {
        let mut i = 0usize;
        while i < self.count {
            match self.bindings[i] {
                Some((k, _)) => match k == var_hash {
                    true => {
                        self.bindings[i] = Some((var_hash, value));
                        return true;
                    }
                    false => {}
                },
                None => {}
            }
            i += 1;
        }

        match self.count < MAX_ENV_VARS {
            true => {
                self.bindings[self.count] = Some((var_hash, value));
                self.count += 1;
                true
            }
            false => false,
        }
    }

    pub fn lookup(&self, var_hash: u32) -> Option<f64> {
        let mut i = 0usize;
        while i < self.count {
            match self.bindings[i] {
                Some((k, val)) => match k == var_hash {
                    true => return Some(val),
                    false => {}
                },
                None => {}
            }
            i += 1;
        }
        None
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SymbolicArena {
    pub nodes: [SymbolicOp; MAX_AST_NODES],
    pub len: usize,
}

impl Default for SymbolicArena {
    fn default() -> Self {
        Self::new()
    }
}

impl SymbolicArena {
    pub const fn new() -> Self {
        Self {
            nodes: [SymbolicOp::Const(0.0); MAX_AST_NODES],
            len: 0,
        }
    }

    pub fn alloc(&mut self, op: SymbolicOp) -> Result<usize, &'static str> {
        match self.len < MAX_AST_NODES {
            true => {
                let idx = self.len;
                self.nodes[idx] = op;
                self.len += 1;
                Ok(idx)
            }
            false => Err("Symbolic AST arena capacity exhausted"),
        }
    }

    pub fn constant(&mut self, c: f64) -> Result<usize, &'static str> {
        self.alloc(SymbolicOp::Const(c))
    }

    pub fn variable(&mut self, var_hash: u32) -> Result<usize, &'static str> {
        self.alloc(SymbolicOp::Var(var_hash))
    }

    pub fn is_zero_node(&self, idx: usize) -> bool {
        match idx < self.len {
            true => match self.nodes[idx] {
                SymbolicOp::Const(c) => c.abs() < EPSILON,
                _ => false,
            },
            false => false,
        }
    }

    pub fn is_one_node(&self, idx: usize) -> bool {
        match idx < self.len {
            true => match self.nodes[idx] {
                SymbolicOp::Const(c) => (c - 1.0).abs() < EPSILON,
                _ => false,
            },
            false => false,
        }
    }

    pub fn nodes_structurally_equal(&self, a: usize, b: usize, depth: usize) -> bool {
        match depth > MAX_TREE_DEPTH || a >= self.len || b >= self.len {
            true => false,
            false => match (self.nodes[a], self.nodes[b]) {
                (SymbolicOp::Const(c1), SymbolicOp::Const(c2)) => (c1 - c2).abs() < EPSILON,
                (SymbolicOp::Var(v1), SymbolicOp::Var(v2)) => v1 == v2,
                (SymbolicOp::Neg(u1), SymbolicOp::Neg(u2))
                | (SymbolicOp::Sqrt(u1), SymbolicOp::Sqrt(u2)) => {
                    self.nodes_structurally_equal(u1, u2, depth + 1)
                }
                (SymbolicOp::Add(l1, r1), SymbolicOp::Add(l2, r2))
                | (SymbolicOp::Sub(l1, r1), SymbolicOp::Sub(l2, r2))
                | (SymbolicOp::Mul(l1, r1), SymbolicOp::Mul(l2, r2))
                | (SymbolicOp::Div(l1, r1), SymbolicOp::Div(l2, r2))
                | (SymbolicOp::Pow(l1, r1), SymbolicOp::Pow(l2, r2)) => {
                    self.nodes_structurally_equal(l1, l2, depth + 1)
                        && self.nodes_structurally_equal(r1, r2, depth + 1)
                }
                _ => false,
            },
        }
    }

    pub fn simplify_node(&mut self, root: usize, depth: usize) -> Result<usize, &'static str> {
        match depth > MAX_TREE_DEPTH || root >= self.len {
            true => return Err("Maximum symbolic AST depth exceeded"),
            false => {}
        }

        let op = self.nodes[root];
        match op {
            SymbolicOp::Const(_) | SymbolicOp::Var(_) => Ok(root),
            SymbolicOp::Neg(inner) => {
                let s_in = self.simplify_node(inner, depth + 1)?;
                match self.nodes[s_in] {
                    SymbolicOp::Const(c) => self.constant(-c),
                    SymbolicOp::Neg(nested) => Ok(nested),
                    _ => match self.is_zero_node(s_in) {
                        true => self.constant(0.0),
                        false => self.alloc(SymbolicOp::Neg(s_in)),
                    },
                }
            }
            SymbolicOp::Sqrt(inner) => {
                let s_in = self.simplify_node(inner, depth + 1)?;
                match self.nodes[s_in] {
                    SymbolicOp::Const(c) => match c >= 0.0 {
                        true => self.constant(c.sqrt()),
                        false => Err("Domain error: negative radicand in symbolic square root"),
                    },
                    _ => match (self.is_zero_node(s_in), self.is_one_node(s_in)) {
                        (true, _) => self.constant(0.0),
                        (false, true) => self.constant(1.0),
                        (false, false) => self.alloc(SymbolicOp::Sqrt(s_in)),
                    },
                }
            }
            SymbolicOp::Add(lhs, rhs) => {
                let l = self.simplify_node(lhs, depth + 1)?;
                let r = self.simplify_node(rhs, depth + 1)?;
                match (self.nodes[l], self.nodes[r]) {
                    (SymbolicOp::Const(a), SymbolicOp::Const(b)) => self.constant(a + b),
                    _ => match (self.is_zero_node(l), self.is_zero_node(r)) {
                        (true, _) => Ok(r),
                        (false, true) => Ok(l),
                        (false, false) => self.alloc(SymbolicOp::Add(l, r)),
                    },
                }
            }
            SymbolicOp::Sub(lhs, rhs) => {
                let l = self.simplify_node(lhs, depth + 1)?;
                let r = self.simplify_node(rhs, depth + 1)?;
                match self.nodes_structurally_equal(l, r, 0) {
                    true => self.constant(0.0),
                    false => match (self.nodes[l], self.nodes[r]) {
                        (SymbolicOp::Const(a), SymbolicOp::Const(b)) => self.constant(a - b),
                        _ => match (self.is_zero_node(r), self.is_zero_node(l)) {
                            (true, _) => Ok(l),
                            (false, true) => self.alloc(SymbolicOp::Neg(r)),
                            (false, false) => self.alloc(SymbolicOp::Sub(l, r)),
                        },
                    },
                }
            }
            SymbolicOp::Mul(lhs, rhs) => {
                let l = self.simplify_node(lhs, depth + 1)?;
                let r = self.simplify_node(rhs, depth + 1)?;
                match (self.nodes[l], self.nodes[r]) {
                    (SymbolicOp::Const(a), SymbolicOp::Const(b)) => self.constant(a * b),
                    _ => match (
                        self.is_zero_node(l) || self.is_zero_node(r),
                        self.is_one_node(l),
                        self.is_one_node(r),
                    ) {
                        (true, _, _) => self.constant(0.0),
                        (false, true, _) => Ok(r),
                        (false, false, true) => Ok(l),
                        (false, false, false) => self.alloc(SymbolicOp::Mul(l, r)),
                    },
                }
            }
            SymbolicOp::Div(lhs, rhs) => {
                let l = self.simplify_node(lhs, depth + 1)?;
                let r = self.simplify_node(rhs, depth + 1)?;
                match self.is_zero_node(r) {
                    true => Err("Zero denominator detected in symbolic division"),
                    false => match self.nodes_structurally_equal(l, r, 0) {
                        true => self.constant(1.0),
                        false => match (self.nodes[l], self.nodes[r]) {
                            (SymbolicOp::Const(a), SymbolicOp::Const(b)) => self.constant(a / b),
                            _ => match (self.is_one_node(r), self.is_zero_node(l)) {
                                (true, _) => Ok(l),
                                (false, true) => self.constant(0.0),
                                (false, false) => self.alloc(SymbolicOp::Div(l, r)),
                            },
                        },
                    },
                }
            }
            SymbolicOp::Pow(lhs, rhs) => {
                let l = self.simplify_node(lhs, depth + 1)?;
                let r = self.simplify_node(rhs, depth + 1)?;
                match (self.nodes[l], self.nodes[r]) {
                    (SymbolicOp::Const(a), SymbolicOp::Const(b)) => self.constant(a.powf(b)),
                    _ => match (
                        self.is_zero_node(r),
                        self.is_one_node(r),
                        self.is_one_node(l),
                        self.is_zero_node(l),
                    ) {
                        (true, _, _, _) => self.constant(1.0),
                        (false, true, _, _) => Ok(l),
                        (false, false, true, _) => self.constant(1.0),
                        (false, false, false, true) => self.constant(0.0),
                        (false, false, false, false) => self.alloc(SymbolicOp::Pow(l, r)),
                    },
                }
            }
        }
    }

    pub fn diff(&mut self, root: usize, target_var: u32, depth: usize) -> Result<usize, &'static str> {
        match depth > MAX_TREE_DEPTH || root >= self.len {
            true => return Err("Symbolic differentiation recursion limit exceeded"),
            false => {}
        }

        let op = self.nodes[root];
        match op {
            SymbolicOp::Const(_) => self.constant(0.0),
            SymbolicOp::Var(v) => match v == target_var {
                true => self.constant(1.0),
                false => self.constant(0.0),
            },
            SymbolicOp::Neg(inner) => {
                let d_in = self.diff(inner, target_var, depth + 1)?;
                self.alloc(SymbolicOp::Neg(d_in))
            }
            SymbolicOp::Sqrt(inner) => {
                let u_prime = self.diff(inner, target_var, depth + 1)?;
                let two = self.constant(2.0)?;
                let sqrt_u = self.alloc(SymbolicOp::Sqrt(inner))?;
                let denom = self.alloc(SymbolicOp::Mul(two, sqrt_u))?;
                self.alloc(SymbolicOp::Div(u_prime, denom))
            }
            SymbolicOp::Add(lhs, rhs) => {
                let dl = self.diff(lhs, target_var, depth + 1)?;
                let dr = self.diff(rhs, target_var, depth + 1)?;
                self.alloc(SymbolicOp::Add(dl, dr))
            }
            SymbolicOp::Sub(lhs, rhs) => {
                let dl = self.diff(lhs, target_var, depth + 1)?;
                let dr = self.diff(rhs, target_var, depth + 1)?;
                self.alloc(SymbolicOp::Sub(dl, dr))
            }
            SymbolicOp::Mul(lhs, rhs) => {
                let dl = self.diff(lhs, target_var, depth + 1)?;
                let dr = self.diff(rhs, target_var, depth + 1)?;
                let t1 = self.alloc(SymbolicOp::Mul(dl, rhs))?;
                let t2 = self.alloc(SymbolicOp::Mul(lhs, dr))?;
                self.alloc(SymbolicOp::Add(t1, t2))
            }
            SymbolicOp::Div(lhs, rhs) => {
                let dl = self.diff(lhs, target_var, depth + 1)?;
                let dr = self.diff(rhs, target_var, depth + 1)?;
                let t1 = self.alloc(SymbolicOp::Mul(dl, rhs))?;
                let t2 = self.alloc(SymbolicOp::Mul(lhs, dr))?;
                let num = self.alloc(SymbolicOp::Sub(t1, t2))?;
                let two = self.constant(2.0)?;
                let denom = self.alloc(SymbolicOp::Pow(rhs, two))?;
                self.alloc(SymbolicOp::Div(num, denom))
            }
            SymbolicOp::Pow(base, exp) => {
                let one = self.constant(1.0)?;
                let exp_minus_1 = self.alloc(SymbolicOp::Sub(exp, one))?;
                let base_pow = self.alloc(SymbolicOp::Pow(base, exp_minus_1))?;
                let coeff = self.alloc(SymbolicOp::Mul(exp, base_pow))?;
                let d_base = self.diff(base, target_var, depth + 1)?;
                self.alloc(SymbolicOp::Mul(coeff, d_base))
            }
        }
    }

    pub fn eval(&self, root: usize, env: &VarEnv, depth: usize) -> Result<f64, &'static str> {
        match depth > MAX_TREE_DEPTH || root >= self.len {
            true => return Err("Symbolic evaluation depth limit exceeded"),
            false => {}
        }

        let val = match self.nodes[root] {
            SymbolicOp::Const(c) => c,
            SymbolicOp::Var(v) => env
                .lookup(v)
                .ok_or("Unbound variable in symbolic evaluation environment")?,
            SymbolicOp::Neg(inner) => -self.eval(inner, env, depth + 1)?,
            SymbolicOp::Sqrt(inner) => {
                let v = self.eval(inner, env, depth + 1)?;
                match v < 0.0 {
                    true => return Err("Domain error: square root of negative scalar"),
                    false => v.sqrt(),
                }
            }
            SymbolicOp::Add(l, r) => {
                self.eval(l, env, depth + 1)? + self.eval(r, env, depth + 1)?
            }
            SymbolicOp::Sub(l, r) => {
                self.eval(l, env, depth + 1)? - self.eval(r, env, depth + 1)?
            }
            SymbolicOp::Mul(l, r) => {
                self.eval(l, env, depth + 1)? * self.eval(r, env, depth + 1)?
            }
            SymbolicOp::Div(l, r) => {
                let denom = self.eval(r, env, depth + 1)?;
                match denom.abs() < 1e-15 {
                    true => return Err("Zero division error in symbolic evaluation"),
                    false => self.eval(l, env, depth + 1)? / denom,
                }
            }
            SymbolicOp::Pow(l, r) => {
                let b = self.eval(l, env, depth + 1)?;
                let e = self.eval(r, env, depth + 1)?;
                b.powf(e)
            }
        };

        match val.is_finite() {
            true => Ok(val),
            false => Err("Non-finite floating-point overflow in symbolic evaluation"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbolic_arena_differentiation_and_simplification() {
        let mut arena = SymbolicArena::new();
        let var_x = 0x01u32;
        let x = arena.variable(var_x).unwrap();
        let three = arena.constant(3.0).unwrap();
        let x_cubed = arena.alloc(SymbolicOp::Pow(x, three)).unwrap();

        let deriv = arena.diff(x_cubed, var_x, 0).unwrap();
        let simplified = arena.simplify_node(deriv, 0).unwrap();

        let mut env = VarEnv::new();
        env.bind(var_x, 2.0);
        let result = arena.eval(simplified, &env, 0).unwrap();
        assert!((result - 12.0).abs() < 1e-9);
    }
}