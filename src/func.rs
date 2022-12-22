use std::{borrow::Borrow, iter::repeat_with, ops::Mul, slice::Iter};

pub trait Function {
    fn eval(&self, t: f32) -> f32;
}

impl<T: Borrow<[f32]>> Function for T {
    #[inline(always)]
    fn eval(&self, t: f32) -> f32 {
        let f: &[f32] = self.borrow();
        f.eval(t)
    }
}

impl Function for [f32] {
    #[inline(always)]
    fn eval(&self, t: f32) -> f32 {
        if self.len() == 1 {
            return self[0];
        }
        self.iter().zip(ts(t)).map(|(&l, r)| l * r).sum()
    }
}

pub fn derive_polynomial<
    I: IntoIterator<Item = T>,
    T: Borrow<N>,
    N: Mul<f32, Output = N> + Clone,
>(
    poly: I,
) -> impl Iterator<Item = N> {
    (1..)
        .map(|i| i as f32)
        .zip(poly.into_iter().skip(1))
        .map(|(l, r)| r.borrow().clone() * l)
}

pub(crate) fn ts(t: f32) -> impl Iterator<Item = f32> {
    let mut t_term = 1.0;
    repeat_with(move || {
        let term = t_term;
        t_term = term * t;
        term
    })
}

pub fn scale_polynomial<
    I: IntoIterator<Item = T>,
    T: Borrow<N>,
    N: Mul<f32, Output = N> + Clone,
>(
    poly: I,
    scale: f32,
) -> impl Iterator<Item = N> {
    poly.into_iter()
        .map(move |coef| coef.borrow().clone() * scale)
}

pub fn stretch_polynomial<
    I: IntoIterator<Item = T>,
    T: Borrow<N>,
    N: Mul<f32, Output = N> + Clone,
>(
    poly: I,
    stretch: f32,
) -> impl Iterator<Item = N> {
    ts(1. / stretch)
        .zip(poly)
        .map(move |(scale, coef)| coef.borrow().clone() * scale)
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Wave<F, A> {
    pub freq: F,
    pub amp: A,
    pub phase: f32,
}
impl<F: Function, A: Function> Function for Wave<F, A> {
    fn eval(&self, t: f32) -> f32 {
        self.amp.eval(t) * (std::f32::consts::TAU * (t + self.phase) * self.freq.eval(t)).sin()
    }
}
impl<'a> Default for Wave<&'a [f32], &'a [f32]> {
    fn default() -> Self {
        Wave {
            freq: &[],
            amp: &[],
            phase: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MultiPoly<'a> {
    pub(crate) coeffs: &'a [f32],
    pub(crate) run_lengths: Iter<'a, u8>,
}
impl<'a> MultiPoly<'a> {
    pub fn new(coeffs: &'a [f32], run_lengths: &'a [u8]) -> Option<Self> {
        (coeffs.len() == run_lengths.iter().cloned().map(usize::from).sum()).then_some(Self {
            coeffs,
            run_lengths: run_lengths.iter(),
        })
    }
}
impl<'a> Iterator for MultiPoly<'a> {
    type Item = &'a [f32];

    fn next(&mut self) -> Option<Self::Item> {
        let len = *self.run_lengths.next()?;
        let (seg, next) = self.coeffs.split_at(len as usize);
        self.coeffs = next;
        Some(seg)
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.run_lengths.size_hint()
    }
}
