//! 3D/4D differential and polyhedral geometry, Hopf fibration S^3 -> S^2 mapping,
//! and stack-allocated Bresenham wireframe rasterization.

pub const MAX_CANVAS_WIDTH: usize = 80;
pub const MAX_CANVAS_HEIGHT: usize = 24;
pub const MAX_WIREFRAME_EDGES: usize = 64;
pub const MAX_BRESENHAM_STEPS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3D {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point2D {
    pub x: f64,
    pub y: f64,
}

impl Point2D {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Line3D {
    pub start: Point3D,
    pub end: Point3D,
}

impl Line3D {
    pub const fn new(start: Point3D, end: Point3D) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialTopologyKind {
    UnitCube,
    Tetrahedron,
    SquarePyramid,
    Octahedron,
    VectorAndLine3D,
    AsymmetricPolytope,
    SpiderwebGeodesicMesh,
}

impl Default for SpatialTopologyKind {
    fn default() -> Self {
        Self::Octahedron
    }
}

impl SpatialTopologyKind {
    pub fn from_query(text: &str) -> Self {
        let lower = text.to_lowercase();
        let is_web = lower.contains("spiderweb")
            || lower.contains("geodesic")
            || lower.contains("hopf")
            || lower.contains("fibration")
            || lower.contains("manifold");
        let is_asym = lower.contains("asymmetric") || lower.contains("chiral") || lower.contains("oblique");
        let is_vec = lower.contains("vector_line") || lower.contains("line3d");
        let is_tetra = lower.contains("tetrahedron") || lower.contains("simplex");
        let is_pyra = lower.contains("pyramid");
        let is_octa = lower.contains("octahedron");

        match (is_web, is_asym, is_vec, is_tetra, is_pyra, is_octa) {
            (true, _, _, _, _, _) => Self::SpiderwebGeodesicMesh,
            (false, true, _, _, _, _) => Self::AsymmetricPolytope,
            (false, false, true, _, _, _) => Self::VectorAndLine3D,
            (false, false, false, true, _, _) => Self::Tetrahedron,
            (false, false, false, false, true, _) => Self::SquarePyramid,
            (false, false, false, false, false, true) => Self::Octahedron,
            _ => Self::UnitCube,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HopfFibrationProjection {
    pub s3_normalized: [f64; 4],
    pub s2_coordinates: [f64; 3],
    pub s3_norm_sq: f64,
    pub s2_norm_sq: f64,
    pub hopf_invariant: i32,
    pub euler_characteristic: i32,
    pub hyper_volume_s3: f64,
}

/// Computes the topological Hopf fibration $h: S^3 \to S^2$ for a 4-vector $(a_0, a_1, a_2, a_3)$
/// and radius parameter $R = a$.
pub fn compute_hopf_fibration_stack(raw_s3: [f64; 4], radius_a: f64) -> HopfFibrationProjection {
    let norm_sq_raw =
        raw_s3[0] * raw_s3[0] + raw_s3[1] * raw_s3[1] + raw_s3[2] * raw_s3[2] + raw_s3[3] * raw_s3[3];
    let norm = norm_sq_raw.sqrt().max(1e-12);
    let a0 = raw_s3[0] / norm;
    let a1 = raw_s3[1] / norm;
    let a2 = raw_s3[2] / norm;
    let a3 = raw_s3[3] / norm;

    let s3_norm_sq = a0 * a0 + a1 * a1 + a2 * a2 + a3 * a3;
    let x_s2 = 2.0 * (a0 * a2 + a1 * a3);
    let y_s2 = 2.0 * (a1 * a2 - a0 * a3);
    let z_s2 = (a0 * a0 + a1 * a1) - (a2 * a2 + a3 * a3);
    let s2_norm_sq = x_s2 * x_s2 + y_s2 * y_s2 + z_s2 * z_s2;

    let pi = std::f64::consts::PI;
    let hyper_volume_s3 = 2.0 * pi * pi * radius_a.powi(3);

    HopfFibrationProjection {
        s3_normalized: [a0, a1, a2, a3],
        s2_coordinates: [x_s2, y_s2, z_s2],
        s3_norm_sq,
        s2_norm_sq,
        hopf_invariant: 1,
        euler_characteristic: 2,
        hyper_volume_s3,
    }
}

pub struct SpatialWireframeBuilder;

impl SpatialWireframeBuilder {
    pub const DUMMY_LINE: Line3D =
        Line3D::new(Point3D::new(0.0, 0.0, 0.0), Point3D::new(0.0, 0.0, 0.0));

    pub fn build_topology(kind: SpatialTopologyKind) -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        match kind {
            SpatialTopologyKind::UnitCube => Self::build_unit_cube(),
            SpatialTopologyKind::Tetrahedron => Self::build_tetrahedron(),
            SpatialTopologyKind::SquarePyramid => Self::build_square_pyramid(),
            SpatialTopologyKind::Octahedron => Self::build_octahedron(),
            SpatialTopologyKind::VectorAndLine3D => Self::build_octahedron(),
            SpatialTopologyKind::AsymmetricPolytope => Self::build_tetrahedron(),
            SpatialTopologyKind::SpiderwebGeodesicMesh => Self::build_spiderweb_mesh(),
        }
    }

    pub fn build_unit_cube() -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        let mut edges = [Self::DUMMY_LINE; MAX_WIREFRAME_EDGES];
        let cube = [
            Line3D::new(Point3D::new(-1.0, -1.0, -1.0), Point3D::new(1.0, -1.0, -1.0)),
            Line3D::new(Point3D::new(1.0, -1.0, -1.0), Point3D::new(1.0, 1.0, -1.0)),
            Line3D::new(Point3D::new(1.0, 1.0, -1.0), Point3D::new(-1.0, 1.0, -1.0)),
            Line3D::new(Point3D::new(-1.0, 1.0, -1.0), Point3D::new(-1.0, -1.0, -1.0)),
            Line3D::new(Point3D::new(-1.0, -1.0, 1.0), Point3D::new(1.0, -1.0, 1.0)),
            Line3D::new(Point3D::new(1.0, -1.0, 1.0), Point3D::new(1.0, 1.0, 1.0)),
            Line3D::new(Point3D::new(1.0, 1.0, 1.0), Point3D::new(-1.0, 1.0, 1.0)),
            Line3D::new(Point3D::new(-1.0, 1.0, 1.0), Point3D::new(-1.0, -1.0, 1.0)),
            Line3D::new(Point3D::new(-1.0, -1.0, -1.0), Point3D::new(-1.0, -1.0, 1.0)),
            Line3D::new(Point3D::new(1.0, -1.0, -1.0), Point3D::new(1.0, -1.0, 1.0)),
            Line3D::new(Point3D::new(1.0, 1.0, -1.0), Point3D::new(1.0, 1.0, 1.0)),
            Line3D::new(Point3D::new(-1.0, 1.0, -1.0), Point3D::new(-1.0, 1.0, 1.0)),
        ];
        let mut i = 0usize;
        while i < 12 {
            edges[i] = cube[i];
            i += 1;
        }
        (edges, 12)
    }

    pub fn build_tetrahedron() -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        let mut edges = [Self::DUMMY_LINE; MAX_WIREFRAME_EDGES];
        let v0 = Point3D::new(0.0, 1.3, 0.0);
        let v1 = Point3D::new(-1.2, -0.9, 1.0);
        let v2 = Point3D::new(1.2, -0.9, 1.0);
        let v3 = Point3D::new(0.0, -0.9, -1.3);

        edges[0] = Line3D::new(v0, v1);
        edges[1] = Line3D::new(v0, v2);
        edges[2] = Line3D::new(v0, v3);
        edges[3] = Line3D::new(v1, v2);
        edges[4] = Line3D::new(v2, v3);
        edges[5] = Line3D::new(v3, v1);
        (edges, 6)
    }

    pub fn build_square_pyramid() -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        let mut edges = [Self::DUMMY_LINE; MAX_WIREFRAME_EDGES];
        let apex = Point3D::new(0.0, 1.4, 0.0);
        let b0 = Point3D::new(-1.2, -1.0, -1.2);
        let b1 = Point3D::new(1.2, -1.0, -1.2);
        let b2 = Point3D::new(1.2, -1.0, 1.2);
        let b3 = Point3D::new(-1.2, -1.0, 1.2);

        edges[0] = Line3D::new(b0, b1);
        edges[1] = Line3D::new(b1, b2);
        edges[2] = Line3D::new(b2, b3);
        edges[3] = Line3D::new(b3, b0);
        edges[4] = Line3D::new(apex, b0);
        edges[5] = Line3D::new(apex, b1);
        edges[6] = Line3D::new(apex, b2);
        edges[7] = Line3D::new(apex, b3);
        (edges, 8)
    }

    pub fn build_octahedron() -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        let mut edges = [Self::DUMMY_LINE; MAX_WIREFRAME_EDGES];
        let top = Point3D::new(0.0, 1.5, 0.0);
        let bot = Point3D::new(0.0, -1.5, 0.0);
        let m0 = Point3D::new(-1.3, 0.0, 0.0);
        let m1 = Point3D::new(0.0, 0.0, 1.3);
        let m2 = Point3D::new(1.3, 0.0, 0.0);
        let m3 = Point3D::new(0.0, 0.0, -1.3);

        edges[0] = Line3D::new(m0, m1);
        edges[1] = Line3D::new(m1, m2);
        edges[2] = Line3D::new(m2, m3);
        edges[3] = Line3D::new(m3, m0);
        edges[4] = Line3D::new(top, m0);
        edges[5] = Line3D::new(top, m1);
        edges[6] = Line3D::new(top, m2);
        edges[7] = Line3D::new(top, m3);
        edges[8] = Line3D::new(bot, m0);
        edges[9] = Line3D::new(bot, m1);
        edges[10] = Line3D::new(bot, m2);
        edges[11] = Line3D::new(bot, m3);
        (edges, 12)
    }

    pub fn build_spiderweb_mesh() -> ([Line3D; MAX_WIREFRAME_EDGES], usize) {
        let mut edges = [Self::DUMMY_LINE; MAX_WIREFRAME_EDGES];
        let mut count = 0usize;
        let dirs: [(f64, f64); 8] = [
            (1.0, 0.0),
            (0.70710678, 0.70710678),
            (0.0, 1.0),
            (-0.70710678, 0.70710678),
            (-1.0, 0.0),
            (-0.70710678, -0.70710678),
            (0.0, -1.0),
            (0.70710678, -0.70710678),
        ];
        let center_hub = Point3D::new(0.0, 0.35, 0.0);
        let mut s = 0usize;
        while s < 8 {
            let (dx, dz) = dirs[s];
            edges[count] = Line3D::new(center_hub, Point3D::new(dx * 1.5, -0.35, dz * 1.5));
            count += 1;
            s += 1;
        }
        let radii: [(f64, f64); 2] = [(0.75, 0.0), (1.5, -0.35)];
        let mut r_idx = 0usize;
        while r_idx < 2 {
            let (rad, y_lvl) = radii[r_idx];
            let mut k = 0usize;
            while k < 8 {
                let next_k = (k + 1) % 8;
                edges[count] = Line3D::new(
                    Point3D::new(dirs[k].0 * rad, y_lvl, dirs[k].1 * rad),
                    Point3D::new(dirs[next_k].0 * rad, y_lvl, dirs[next_k].1 * rad),
                );
                count += 1;
                k += 1;
            }
            r_idx += 1;
        }
        (edges, count)
    }
}

pub fn project_3d_to_2d(p: Point3D, yaw_deg: f64, pitch_deg: f64, distance: f64) -> Point2D {
    let yaw_rad = yaw_deg.to_radians();
    let pitch_rad = pitch_deg.to_radians();

    let cos_y = yaw_rad.cos();
    let sin_y = yaw_rad.sin();
    let cos_p = pitch_rad.cos();
    let sin_p = pitch_rad.sin();

    let x1 = p.x * cos_y + p.z * sin_y;
    let y1 = p.y;
    let z1 = -p.x * sin_y + p.z * cos_y;

    let x2 = x1;
    let y2 = y1 * cos_p - z1 * sin_p;
    let z2 = y1 * sin_p + z1 * cos_p;

    let dist = match distance <= 0.0 {
        true => 1.0,
        false => distance,
    };
    let denom = z2 + dist;
    let factor = match denom.abs() < 1e-4 {
        true => dist / 1e-4,
        false => dist / denom,
    };

    Point2D {
        x: x2 * factor,
        y: y2 * factor,
    }
}

pub struct AsciiCanvas {
    pub width: usize,
    pub height: usize,
    buffer: [[char; MAX_CANVAS_WIDTH]; MAX_CANVAS_HEIGHT],
}

impl AsciiCanvas {
    pub fn new(width: usize, height: usize) -> Self {
        let w = width.clamp(1, MAX_CANVAS_WIDTH);
        let h = height.clamp(1, MAX_CANVAS_HEIGHT);
        Self {
            width: w,
            height: h,
            buffer: [[' '; MAX_CANVAS_WIDTH]; MAX_CANVAS_HEIGHT],
        }
    }

    pub fn set_pixel(&mut self, x: i32, y: i32, ch: char) {
        let in_bounds = x >= 0 && x < (self.width as i32) && y >= 0 && y < (self.height as i32);
        match in_bounds {
            true => self.buffer[y as usize][x as usize] = ch,
            false => {}
        }
    }

    pub fn draw_line(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, ch: char) {
        let dx = (x1 - x0).abs();
        let sx = match x0 < x1 {
            true => 1,
            false => -1,
        };
        let dy = -(y1 - y0).abs();
        let sy = match y0 < y1 {
            true => 1,
            false => -1,
        };
        let mut err = dx + dy;
        let mut step_budget = MAX_BRESENHAM_STEPS;

        while step_budget > 0 {
            step_budget -= 1;
            self.set_pixel(x0, y0, ch);
            match x0 == x1 && y0 == y1 {
                true => break,
                false => {}
            }
            let e2 = 2 * err;
            match e2 >= dy {
                true => {
                    err += dy;
                    x0 += sx;
                }
                false => {}
            }
            match e2 <= dx {
                true => {
                    err += dx;
                    y0 += sy;
                }
                false => {}
            }
        }
    }

    pub fn render_topology_3d(&mut self, kind: SpatialTopologyKind, yaw: f64, pitch: f64) -> String {
        let (edges, count) = SpatialWireframeBuilder::build_topology(kind);
        let mut projected = [(Point2D::new(0.0, 0.0), Point2D::new(0.0, 0.0)); MAX_WIREFRAME_EDGES];
        let mut min_x = f64::MAX;
        let mut max_x = f64::MIN;
        let mut min_y = f64::MAX;
        let mut max_y = f64::MIN;

        let mut i = 0usize;
        while i < count {
            let p1 = project_3d_to_2d(edges[i].start, yaw, pitch, 5.0);
            let p2 = project_3d_to_2d(edges[i].end, yaw, pitch, 5.0);
            min_x = min_x.min(p1.x).min(p2.x);
            max_x = max_x.max(p1.x).max(p2.x);
            min_y = min_y.min(p1.y).min(p2.y);
            max_y = max_y.max(p1.y).max(p2.y);
            projected[i] = (p1, p2);
            i += 1;
        }

        let span_x = (max_x - min_x).max(1e-4);
        let span_y = (max_y - min_y).max(1e-4);
        let target_w = (self.width.saturating_sub(4)).max(1) as f64;
        let target_h = (self.height.saturating_sub(4)).max(1) as f64;

        let mut k = 0usize;
        while k < count {
            let (p1, p2) = projected[k];
            let x0 = 2 + (((p1.x - min_x) / span_x) * target_w).round() as i32;
            let y0 = 2 + (((max_y - p1.y) / span_y) * target_h).round() as i32;
            let x1 = 2 + (((p2.x - min_x) / span_x) * target_w).round() as i32;
            let y1 = 2 + (((max_y - p2.y) / span_y) * target_h).round() as i32;
            self.draw_line(x0, y0, x1, y1, '#');
            k += 1;
        }

        let mut out = String::with_capacity((self.width + 1) * self.height);
        let mut r = 0usize;
        while r < self.height {
            let mut c = 0usize;
            while c < self.width {
                out.push(self.buffer[r][c]);
                c += 1;
            }
            out.push('\n');
            r += 1;
        }
        out
    }
}