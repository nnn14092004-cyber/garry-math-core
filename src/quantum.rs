//! Stack-allocated complex Hilbert space operator algebra, Lie brackets,
//! Kronecker tensor products, and von Neumann density matrix decoherence.

pub const MAX_QUANTUM_DIM: usize = 8;
pub const MAX_QUANTUM_CELLS: usize = 64;
pub const QUANTUM_EPSILON: f64 = 1e-10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex64 {
    pub re: f64,
    pub im: f64,
}

impl Complex64 {
    pub const fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    pub const fn zero() -> Self {
        Self { re: 0.0, im: 0.0 }
    }

    pub const fn one() -> Self {
        Self { re: 1.0, im: 0.0 }
    }

    pub const fn i() -> Self {
        Self { re: 0.0, im: 1.0 }
    }

    pub const fn conj(&self) -> Self {
        Self {
            re: self.re,
            im: -self.im,
        }
    }

    pub const fn norm_sq(&self) -> f64 {
        self.re * self.re + self.im * self.im
    }

    pub fn abs(&self) -> f64 {
        self.norm_sq().sqrt()
    }

    pub fn div(&self, other: Self) -> Result<Self, &'static str> {
        let denom = other.norm_sq();
        match denom < 1e-15 {
            true => Err("Division by zero in complex field C"),
            false => Ok(Self {
                re: (self.re * other.re + self.im * other.im) / denom,
                im: (self.im * other.re - self.re * other.im) / denom,
            }),
        }
    }

    pub fn approx_eq(&self, other: &Self, eps: f64) -> bool {
        (self.re - other.re).abs() <= eps && (self.im - other.im).abs() <= eps
    }
}

impl std::ops::Add for Complex64 {
    type Output = Self;
    #[inline]
    fn add(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re + rhs.re,
            im: self.im + rhs.im,
        }
    }
}

impl std::ops::Sub for Complex64 {
    type Output = Self;
    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re - rhs.re,
            im: self.im - rhs.im,
        }
    }
}

impl std::ops::Mul for Complex64 {
    type Output = Self;
    #[inline]
    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

impl std::ops::Neg for Complex64 {
    type Output = Self;
    #[inline]
    fn neg(self) -> Self::Output {
        Self {
            re: -self.re,
            im: -self.im,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ComplexMatrix {
    pub rows: usize,
    pub cols: usize,
    pub data: [Complex64; MAX_QUANTUM_CELLS],
}

impl ComplexMatrix {
    pub fn zero(rows: usize, cols: usize) -> Result<Self, &'static str> {
        match rows == 0 || cols == 0 || rows > MAX_QUANTUM_DIM || cols > MAX_QUANTUM_DIM {
            true => Err("Hilbert operator dimensions exceed bounded stack capacity (8x8)"),
            false => Ok(Self {
                rows,
                cols,
                data: [Complex64::zero(); MAX_QUANTUM_CELLS],
            }),
        }
    }

    pub fn from_slice(rows: usize, cols: usize, slice: &[Complex64]) -> Result<Self, &'static str> {
        let mut mat = Self::zero(rows, cols)?;
        let expected = rows * cols;
        match slice.len() != expected {
            true => Err("Slice length does not match operator dimensions"),
            false => {
                let mut i = 0usize;
                while i < expected {
                    mat.data[i] = slice[i];
                    i += 1;
                }
                Ok(mat)
            }
        }
    }

    pub fn identity(n: usize) -> Result<Self, &'static str> {
        let mut mat = Self::zero(n, n)?;
        let mut i = 0usize;
        while i < n {
            mat.data[i * n + i] = Complex64::one();
            i += 1;
        }
        Ok(mat)
    }

    pub const fn get(&self, r: usize, c: usize) -> Option<Complex64> {
        match r < self.rows && c < self.cols {
            true => Some(self.data[r * self.cols + c]),
            false => None,
        }
    }

    pub fn set(&mut self, r: usize, c: usize, val: Complex64) -> Result<(), &'static str> {
        match r < self.rows && c < self.cols {
            true => {
                self.data[r * self.cols + c] = val;
                Ok(())
            }
            false => Err("Matrix index out of bounds"),
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self, &'static str> {
        match self.rows != other.rows || self.cols != other.cols {
            true => Err("Operator dimension mismatch in addition"),
            false => {
                let mut out = Self::zero(self.rows, self.cols)?;
                let total = self.rows * self.cols;
                let mut i = 0usize;
                while i < total {
                    out.data[i] = self.data[i] + other.data[i];
                    i += 1;
                }
                Ok(out)
            }
        }
    }

    pub fn sub(&self, other: &Self) -> Result<Self, &'static str> {
        match self.rows != other.rows || self.cols != other.cols {
            true => Err("Operator dimension mismatch in subtraction"),
            false => {
                let mut out = Self::zero(self.rows, self.cols)?;
                let total = self.rows * self.cols;
                let mut i = 0usize;
                while i < total {
                    out.data[i] = self.data[i] - other.data[i];
                    i += 1;
                }
                Ok(out)
            }
        }
    }

    pub fn scale(&self, factor: Complex64) -> Self {
        let mut out = *self;
        let total = self.rows * self.cols;
        let mut i = 0usize;
        while i < total {
            out.data[i] = self.data[i] * factor;
            i += 1;
        }
        out
    }

    pub fn matmul(&self, other: &Self) -> Result<Self, &'static str> {
        match self.cols != other.rows {
            true => Err("Inner dimension mismatch in operator multiplication"),
            false => {
                let p = self.rows;
                let q = other.cols;
                let k_dim = self.cols;
                let mut out = Self::zero(p, q)?;
                let mut i = 0usize;
                while i < p {
                    let mut k = 0usize;
                    while k < k_dim {
                        let a_ik = self.data[i * self.cols + k];
                        let mut j = 0usize;
                        while j < q {
                            let b_kj = other.data[k * other.cols + j];
                            out.data[i * q + j] = out.data[i * q + j] + (a_ik * b_kj);
                            j += 1;
                        }
                        k += 1;
                    }
                    i += 1;
                }
                Ok(out)
            }
        }
    }

    pub fn adjoint(&self) -> Self {
        let mut out = Self {
            rows: self.cols,
            cols: self.rows,
            data: [Complex64::zero(); MAX_QUANTUM_CELLS],
        };
        let mut r = 0usize;
        while r < self.rows {
            let mut c = 0usize;
            while c < self.cols {
                out.data[c * self.rows + r] = self.data[r * self.cols + c].conj();
                c += 1;
            }
            r += 1;
        }
        out
    }

    pub fn trace(&self) -> Result<Complex64, &'static str> {
        match self.rows == self.cols {
            false => Err("Operator trace is defined exclusively for square matrices"),
            true => {
                let mut tr = Complex64::zero();
                let mut i = 0usize;
                while i < self.rows {
                    tr = tr + self.data[i * self.cols + i];
                    i += 1;
                }
                Ok(tr)
            }
        }
    }

    pub fn kronecker(&self, other: &Self) -> Result<Self, &'static str> {
        let out_rows = self.rows * other.rows;
        let out_cols = self.cols * other.cols;
        let mut out = Self::zero(out_rows, out_cols)?;

        let mut i = 0usize;
        while i < self.rows {
            let mut j = 0usize;
            while j < self.cols {
                let a_ij = self.data[i * self.cols + j];
                let mut k = 0usize;
                while k < other.rows {
                    let mut l = 0usize;
                    while l < other.cols {
                        let b_kl = other.data[k * other.cols + l];
                        let tr = i * other.rows + k;
                        let tc = j * other.cols + l;
                        out.data[tr * out_cols + tc] = a_ij * b_kl;
                        l += 1;
                    }
                    k += 1;
                }
                j += 1;
            }
            i += 1;
        }
        Ok(out)
    }

    pub fn commutator(&self, other: &Self) -> Result<Self, &'static str> {
        let ab = self.matmul(other)?;
        let ba = other.matmul(self)?;
        ab.sub(&ba)
    }

    pub fn anti_commutator(&self, other: &Self) -> Result<Self, &'static str> {
        let ab = self.matmul(other)?;
        let ba = other.matmul(self)?;
        ab.add(&ba)
    }

    pub fn approx_eq(&self, other: &Self, eps: f64) -> bool {
        match self.rows == other.rows && self.cols == other.cols {
            false => false,
            true => {
                let total = self.rows * self.cols;
                let mut i = 0usize;
                while i < total {
                    match self.data[i].approx_eq(&other.data[i], eps) {
                        false => return false,
                        true => {}
                    }
                    i += 1;
                }
                true
            }
        }
    }

    pub fn is_hermitian(&self, eps: f64) -> bool {
        match self.rows == self.cols {
            false => false,
            true => {
                let adj = self.adjoint();
                self.approx_eq(&adj, eps)
            }
        }
    }

    pub fn is_unitary(&self, eps: f64) -> bool {
        match self.rows == self.cols {
            false => false,
            true => {
                let adj = self.adjoint();
                match (adj.matmul(self), Self::identity(self.rows)) {
                    (Ok(prod), Ok(id)) => prod.approx_eq(&id, eps),
                    _ => false,
                }
            }
        }
    }
}

/// Computes outer product density operator $\rho = |\psi\rangle\langle\psi|$ with phase damping $\lambda \in [0, 1]$,
/// returning $(\rho, \text{Tr}(\rho), \gamma = \text{Tr}(\rho^2))$.
pub fn compute_density_and_dephasing_stack(
    amplitudes: &[f64],
    lambda_dephasing: f64,
) -> Result<(ComplexMatrix, f64, f64), &'static str> {
    let dim = amplitudes.len();
    match dim < 2 || dim > MAX_QUANTUM_DIM {
        true => return Err("State vector dimension must be within [2, 8]"),
        false => {}
    }

    let mut norm_sq = 0.0f64;
    let mut i = 0usize;
    while i < dim {
        norm_sq += amplitudes[i] * amplitudes[i];
        i += 1;
    }
    let norm = norm_sq.sqrt().max(1e-12);
    let eta = (1.0 - lambda_dephasing).clamp(0.0, 1.0).sqrt();

    let mut rho = ComplexMatrix::zero(dim, dim)?;
    let mut r = 0usize;
    while r < dim {
        let mut c = 0usize;
        while c < dim {
            let base = (amplitudes[r] / norm) * (amplitudes[c] / norm);
            let damped = match r == c {
                true => base,
                false => base * eta,
            };
            rho.set(r, c, Complex64::new(damped, 0.0))?;
            c += 1;
        }
        r += 1;
    }

    let tr_rho = rho.trace()?.re;
    let rho_sq = rho.matmul(&rho)?;
    let purity = rho_sq.trace()?.re;
    Ok((rho, tr_rho, purity))
}

pub fn pauli_x() -> ComplexMatrix {
    ComplexMatrix::from_slice(
        2,
        2,
        &[
            Complex64::zero(),
            Complex64::one(),
            Complex64::one(),
            Complex64::zero(),
        ],
    )
    .unwrap()
}

pub fn pauli_y() -> ComplexMatrix {
    ComplexMatrix::from_slice(
        2,
        2,
        &[
            Complex64::zero(),
            -Complex64::i(),
            Complex64::i(),
            Complex64::zero(),
        ],
    )
    .unwrap()
}

pub fn pauli_z() -> ComplexMatrix {
    ComplexMatrix::from_slice(
        2,
        2,
        &[
            Complex64::one(),
            Complex64::zero(),
            Complex64::zero(),
            -Complex64::one(),
        ],
    )
    .unwrap()
}

pub fn hadamard() -> ComplexMatrix {
    let s = Complex64::new(1.0 / 2.0f64.sqrt(), 0.0);
    ComplexMatrix::from_slice(2, 2, &[s, s, s, -s]).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pauli_su2_lie_commutator() {
        let px = pauli_x();
        let py = pauli_y();
        let pz = pauli_z();
        let comm = px.commutator(&py).unwrap();
        let expected = pz.scale(Complex64::new(0.0, 2.0));
        assert!(comm.approx_eq(&expected, QUANTUM_EPSILON));
    }
}