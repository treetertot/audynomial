use std::{
    iter::Sum,
    ops::{Add, AddAssign, Mul, Neg, Sub},
};

use crate::func::{derive_polynomial, ts};

#[derive(Debug, Clone, Copy)]
pub struct Vec2(pub [f32; 2]);
impl Vec2 {
    pub fn new(a: f32, b: f32) -> Vec2 {
        Vec2([a, b])
    }
}
impl Default for Vec2 {
    fn default() -> Self {
        0f32.into()
    }
}
impl From<[f32; 2]> for Vec2 {
    fn from(x: [f32; 2]) -> Self {
        Vec2(x)
    }
}
impl From<(f32, f32)> for Vec2 {
    fn from((a, b): (f32, f32)) -> Self {
        Vec2([a, b])
    }
}
impl From<f32> for Vec2 {
    fn from(n: f32) -> Self {
        Vec2([n, n])
    }
}
impl Add for Vec2 {
    type Output = Vec2;

    fn add(self, rhs: Self) -> Self::Output {
        Vec2([self.0[0] + rhs.0[0], self.0[1] + rhs.0[1]])
    }
}
impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Self) {
        for (dest, right) in self.0.iter_mut().zip(rhs.0) {
            *dest = right;
        }
    }
}
impl Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, rhs: f32) -> Self::Output {
        Vec2(self.0.map(|n| n * rhs))
    }
}
impl Sub for Vec2 {
    type Output = Vec2;

    fn sub(self, rhs: Self) -> Self::Output {
        Vec2([self.0[0] - rhs.0[0], self.0[1] - rhs.0[1]])
    }
}
impl Neg for Vec2 {
    type Output = Vec2;

    fn neg(self) -> Self::Output {
        Vec2(self.0.map(Neg::neg))
    }
}
impl Sum for Vec2 {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Vec2::default(), |r, l| r + l)
    }
}

pub struct CubicBezier(pub [Vec2; 4]);
impl CubicBezier {
    pub fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        // grabbed this from Freya Holmér's splines video
        Self([
            p0,
            p0 * -3. + p1 * 3.,
            p0 * 3. + p1 * -6. + p2 * 3.,
            -p0 + p1 * 3. + p2 * -3. + p3,
        ])
    }
    pub fn eval(&self, t: f32) -> Vec2 {
        ts(t).zip(&self.0).map(|(t_term, p)| *p * t_term).sum()
    }
    pub fn derive(&self) -> QuadraticBezier {
        let mut out = [Vec2::default(); 3];
        derive_polynomial(self.0)
            .zip(&mut out)
            .for_each(|(l, r)| *r += l);
        QuadraticBezier(out)
    }
}

pub struct QuadraticBezier(pub [Vec2; 3]);
impl QuadraticBezier {
    pub fn new(p0: Vec2, p1: Vec2, p2: Vec2) -> Self {
        // did this by hand and I am prone to mistakes
        // but it looks like the code above so I feel good
        Self([p0, p0 * 2. + p1 * 2., p0 - p1 * 2. + p2])
    }
    pub fn eval(&self, t: f32) -> Vec2 {
        ts(t).zip(&self.0).map(|(t_term, p)| *p * t_term).sum()
    }
}
