//! Points, rectangles and vector paths in diagram units (centimetres).

use std::ops::{Add, Mul, Sub};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct P {
    pub x: f64,
    pub y: f64,
}

pub const fn p(x: f64, y: f64) -> P {
    P { x, y }
}

impl Add for P {
    type Output = P;
    fn add(self, o: P) -> P {
        p(self.x + o.x, self.y + o.y)
    }
}
impl Sub for P {
    type Output = P;
    fn sub(self, o: P) -> P {
        p(self.x - o.x, self.y - o.y)
    }
}
impl Mul<f64> for P {
    type Output = P;
    fn mul(self, k: f64) -> P {
        p(self.x * k, self.y * k)
    }
}

impl P {
    pub fn len(self) -> f64 {
        (self.x * self.x + self.y * self.y).sqrt()
    }
    pub fn dist(self, o: P) -> f64 {
        (self - o).len()
    }
    pub fn norm(self) -> P {
        let l = self.len();
        if l < 1e-12 {
            p(1.0, 0.0)
        } else {
            p(self.x / l, self.y / l)
        }
    }
    pub fn perp(self) -> P {
        p(-self.y, self.x)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct R {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub const fn r(x: f64, y: f64, w: f64, h: f64) -> R {
    R { x, y, w, h }
}

impl R {
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn center(&self) -> P {
        p(self.cx(), self.cy())
    }
    pub fn at(&self, u: f64, v: f64) -> P {
        p(self.x + self.w * u, self.y + self.h * v)
    }
    pub fn contains(&self, q: P) -> bool {
        q.x >= self.x && q.x <= self.right() && q.y >= self.y && q.y <= self.bottom()
    }
    pub fn contains_rect(&self, o: &R) -> bool {
        o.x >= self.x && o.right() <= self.right() && o.y >= self.y && o.bottom() <= self.bottom()
    }
    pub fn intersects(&self, o: &R) -> bool {
        o.x <= self.right() && o.right() >= self.x && o.y <= self.bottom() && o.bottom() >= self.y
    }
    pub fn inflate(&self, d: f64) -> R {
        r(self.x - d, self.y - d, self.w + 2.0 * d, self.h + 2.0 * d)
    }
    pub fn from_points(a: P, b: P) -> R {
        r(a.x.min(b.x), a.y.min(b.y), (a.x - b.x).abs(), (a.y - b.y).abs())
    }
    pub fn bounding(pts: &[P]) -> R {
        if pts.is_empty() {
            return R::default();
        }
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for q in pts {
            x0 = x0.min(q.x);
            y0 = y0.min(q.y);
            x1 = x1.max(q.x);
            y1 = y1.max(q.y);
        }
        r(x0, y0, x1 - x0, y1 - y0)
    }
    pub fn union(&self, o: &R) -> R {
        R::from_points(
            p(self.x.min(o.x), self.y.min(o.y)),
            p(self.right().max(o.right()), self.bottom().max(o.bottom())),
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Seg {
    Move(P),
    Line(P),
    Curve(P, P, P),
    Close,
}

const KAPPA: f64 = 0.552_284_749_8;

/// A vector path made of moves, lines, cubic curves and closes.
#[derive(Clone, Debug, Default)]
pub struct Path {
    pub segs: Vec<Seg>,
}

impl Path {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn move_to(mut self, x: f64, y: f64) -> Self {
        self.segs.push(Seg::Move(p(x, y)));
        self
    }
    pub fn line_to(mut self, x: f64, y: f64) -> Self {
        self.segs.push(Seg::Line(p(x, y)));
        self
    }
    pub fn curve_to(mut self, x1: f64, y1: f64, x2: f64, y2: f64, x: f64, y: f64) -> Self {
        self.segs.push(Seg::Curve(p(x1, y1), p(x2, y2), p(x, y)));
        self
    }
    pub fn close(mut self) -> Self {
        self.segs.push(Seg::Close);
        self
    }
    pub fn poly(mut self, pts: &[P], close: bool) -> Self {
        if pts.is_empty() {
            return self;
        }
        self.segs.push(Seg::Move(pts[0]));
        for q in &pts[1..] {
            self.segs.push(Seg::Line(*q));
        }
        if close {
            self.segs.push(Seg::Close);
        }
        self
    }
    pub fn polygon(pts: &[P]) -> Self {
        Path::new().poly(pts, true)
    }
    pub fn rect(b: R) -> Self {
        Path::polygon(&[p(b.x, b.y), p(b.right(), b.y), p(b.right(), b.bottom()), p(b.x, b.bottom())])
    }
    pub fn ellipse(b: R) -> Self {
        let (rx, ry, cx, cy) = (b.w / 2.0, b.h / 2.0, b.cx(), b.cy());
        let (kx, ky) = (rx * KAPPA, ry * KAPPA);
        Path::new()
            .move_to(cx + rx, cy)
            .curve_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry)
            .curve_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy)
            .curve_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry)
            .curve_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy)
            .close()
    }
    pub fn round_rect(b: R, rad: f64) -> Self {
        let rad = rad.min(b.w.min(b.h) / 2.0);
        if rad <= 1e-6 {
            return Path::rect(b);
        }
        let k = rad * KAPPA;
        let (x0, y0, x1, y1) = (b.x, b.y, b.right(), b.bottom());
        Path::new()
            .move_to(x0 + rad, y0)
            .line_to(x1 - rad, y0)
            .curve_to(x1 - rad + k, y0, x1, y0 + rad - k, x1, y0 + rad)
            .line_to(x1, y1 - rad)
            .curve_to(x1, y1 - rad + k, x1 - rad + k, y1, x1 - rad, y1)
            .line_to(x0 + rad, y1)
            .curve_to(x0 + rad - k, y1, x0, y1 - rad + k, x0, y1 - rad)
            .line_to(x0, y0 + rad)
            .curve_to(x0, y0 + rad - k, x0 + rad - k, y0, x0 + rad, y0)
            .close()
    }
    pub fn append(mut self, other: Path) -> Self {
        self.segs.extend(other.segs);
        self
    }

    /// Flatten into polylines, each with a "closed" flag.
    pub fn flatten(&self, steps: usize) -> Vec<(Vec<P>, bool)> {
        let mut res: Vec<(Vec<P>, bool)> = Vec::new();
        let mut open = false;
        let mut last = P::default();
        for s in &self.segs {
            match *s {
                Seg::Move(a) => {
                    res.push((vec![a], false));
                    open = true;
                    last = a;
                }
                Seg::Line(a) => {
                    if !open {
                        res.push((vec![last], false));
                        open = true;
                    }
                    res.last_mut().unwrap().0.push(a);
                    last = a;
                }
                Seg::Curve(a, b, c) => {
                    if !open {
                        res.push((vec![last], false));
                        open = true;
                    }
                    let cur = &mut res.last_mut().unwrap().0;
                    for i in 1..=steps {
                        cur.push(bez(last, a, b, c, i as f64 / steps as f64));
                    }
                    last = c;
                }
                Seg::Close => {
                    if open {
                        res.last_mut().unwrap().1 = true;
                        if let Some(first) = res.last().unwrap().0.first() {
                            last = *first;
                        }
                    }
                    open = false;
                }
            }
        }
        res
    }
}

pub fn bez(p0: P, p1: P, p2: P, p3: P, t: f64) -> P {
    let u = 1.0 - t;
    p0 * (u * u * u) + p1 * (3.0 * u * u * t) + p2 * (3.0 * u * t * t) + p3 * (t * t * t)
}

pub fn dist_to_segment(q: P, a: P, b: P) -> f64 {
    let ab = b - a;
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 < 1e-12 {
        return q.dist(a);
    }
    let t = (((q.x - a.x) * ab.x + (q.y - a.y) * ab.y) / l2).clamp(0.0, 1.0);
    q.dist(a + ab * t)
}

pub fn dist_to_polyline(q: P, pts: &[P]) -> f64 {
    if pts.len() == 1 {
        return q.dist(pts[0]);
    }
    pts.windows(2).map(|w| dist_to_segment(q, w[0], w[1])).fold(f64::MAX, f64::min)
}

pub fn point_in_polygon(q: P, poly: &[P]) -> bool {
    let mut inside = false;
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y + 1e-300) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Parameter t along a-b where it crosses c-d, if it does.
pub fn seg_intersect(a: P, b: P, c: P, d: P) -> Option<f64> {
    let rr = b - a;
    let s = d - c;
    let den = rr.x * s.y - rr.y * s.x;
    if den.abs() < 1e-12 {
        return None;
    }
    let ca = c - a;
    let t = (ca.x * s.y - ca.y * s.x) / den;
    let u = (ca.x * rr.y - ca.y * rr.x) / den;
    if !(0.0..=1.0).contains(&t) || !(0.0..=1.0).contains(&u) {
        return None;
    }
    Some(t)
}

fn trim_start(pts: Vec<P>, mut len: f64) -> Vec<P> {
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    if total <= len * 1.05 {
        return pts;
    }
    let mut k = 0;
    while k + 1 < pts.len() {
        let d = pts[k].dist(pts[k + 1]);
        if d >= len {
            let np = pts[k] + (pts[k + 1] - pts[k]) * (len / d.max(1e-12));
            let mut out = vec![np];
            out.extend_from_slice(&pts[k + 1..]);
            return out;
        }
        len -= d;
        k += 1;
    }
    pts
}

/// Shorten a polyline at both ends (to make room for arrowheads).
pub fn trim(pts: &[P], start: f64, end: f64) -> Vec<P> {
    let mut v = pts.to_vec();
    if v.len() < 2 {
        return v;
    }
    if start > 0.0 {
        v = trim_start(v, start);
    }
    if end > 0.0 {
        v.reverse();
        v = trim_start(v, end);
        v.reverse();
    }
    v
}

/// The point `len` back along the polyline from its end.
pub fn point_back_from_end(pts: &[P], mut len: f64) -> P {
    for i in (1..pts.len()).rev() {
        let d = pts[i].dist(pts[i - 1]);
        if d >= len {
            return pts[i] + (pts[i - 1] - pts[i]) * (len / d.max(1e-12));
        }
        len -= d;
    }
    pts[0]
}

pub fn snap(q: P, grid: f64) -> P {
    if grid <= 0.0 {
        q
    } else {
        p((q.x / grid).round() * grid, (q.y / grid).round() * grid)
    }
}

fn circle(a: P, b: P, c: P) -> Option<(P, f64)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let (a2, b2, c2) = (a.x * a.x + a.y * a.y, b.x * b.x + b.y * b.y, c.x * c.x + c.y * c.y);
    let center = p(
        (a2 * (b.y - c.y) + b2 * (c.y - a.y) + c2 * (a.y - b.y)) / d,
        (a2 * (c.x - b.x) + b2 * (a.x - c.x) + c2 * (b.x - a.x)) / d,
    );
    Some((center, center.dist(a)))
}

fn norm_angle(mut a: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    while a < 0.0 {
        a += tau;
    }
    while a >= tau {
        a -= tau;
    }
    a
}

/// Points along a circular arc from a to b that bulges sideways by `curve`.
pub fn arc_points(a: P, b: P, curve: f64, steps: usize) -> Vec<P> {
    if curve.abs() < 1e-4 || a.dist(b) < 1e-6 {
        return vec![a, b];
    }
    let mid = (a + b) * 0.5;
    let dir = (b - a).norm();
    let perp = p(dir.y, -dir.x);
    let m = mid + perp * curve;
    let Some((c, rad)) = circle(a, m, b) else { return vec![a, b] };
    let a0 = (a.y - c.y).atan2(a.x - c.x);
    let am = (m.y - c.y).atan2(m.x - c.x);
    let a1 = (b.y - c.y).atan2(b.x - c.x);
    let mut sweep = norm_angle(a1 - a0);
    if norm_angle(am - a0) > sweep {
        sweep -= std::f64::consts::TAU;
    }
    (0..=steps)
        .map(|i| {
            let t = a0 + sweep * i as f64 / steps as f64;
            p(c.x + rad * t.cos(), c.y + rad * t.sin())
        })
        .collect()
}
