use glam::Vec4;
use palette::Srgba;

use crate::layer::brush::Draw;

#[derive(Clone)]
pub struct BrushParam<T> {
    pub inner: ParamCurve<T>,
    pub scale: f32,
    pub offset: f32,
}

#[derive(Clone, Copy)]
pub enum ParamCurve<T> {
    Constant { val: T },
    ForceIndex { min: T, max: T, idx: T },
}

/// Stable identity of an adjustable brush parameter.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum BrushParamKey {
    Size,
    Flow,
    Softness,
    Spacing,
    Sigma,
    ColorRatio,
    SampleRadius,
    SampleRate,
    Color,
    Erase,
}

/// Borrowed, type-erased view of a brush parameter. Adding a new parameter type means adding a
/// variant here plus a field and one macro entry on the brush that exposes it.
pub enum BrushValue<'a> {
    Scalar(&'a BrushParam<f32>),
    Vec4(&'a Vec4),
    Color(&'a Srgba),
    Toggle(&'a bool),
}

pub enum BrushValueMut<'a> {
    Scalar(&'a mut BrushParam<f32>),
    Vec4(&'a mut Vec4),
    Color(&'a mut Srgba),
    Toggle(&'a mut bool),
}

impl BrushParam<f32> {
    pub const fn constant(val: f32) -> BrushParam<f32> {
        BrushParam {
            inner: ParamCurve::Constant { val: 1.0 },
            scale: val,
            offset: 0.0,
        }
    }

    pub const fn force_index(min: f32, max: f32, idx: f32) -> BrushParam<f32> {
        BrushParam {
            inner: ParamCurve::ForceIndex {
                min: min / max,
                max: 1.0,
                idx,
            },
            scale: max,
            offset: 0.0,
        }
    }

    pub fn get(&self, draw: Draw) -> f32 {
        let raw = match self.inner {
            ParamCurve::Constant { val: value } => value,
            ParamCurve::ForceIndex { min, max, idx } => min + (max - min) * draw.force.powf(idx),
        };
        raw * self.scale + self.offset
    }
}

impl std::ops::Add<f32> for BrushParam<f32> {
    type Output = Self;

    fn add(self, rhs: f32) -> Self::Output {
        BrushParam {
            offset: self.offset + rhs,
            ..self
        }
    }
}

impl std::ops::Sub<f32> for BrushParam<f32> {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self::Output {
        BrushParam {
            offset: self.offset - rhs,
            ..self
        }
    }
}

impl std::ops::Mul<f32> for BrushParam<f32> {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        BrushParam {
            scale: self.scale * rhs,
            ..self
        }
    }
}

impl std::ops::Div<f32> for BrushParam<f32> {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        BrushParam {
            scale: self.scale / rhs,
            ..self
        }
    }
}

/// Smallest distance between two dabs. Keeps [`step_of`] strictly positive so the interpolation
/// loop can never stall on a degenerate zero-radius brush.
pub const MIN_STEP: f32 = 0.1;

/// Distance between consecutive dabs for a linear brush: `spacing` is expressed as a fraction of
/// the brush diameter.
pub fn step_of(size: f32, spacing: f32) -> f32 {
    (2.0 * size.max(0.0) * spacing.max(0.0)).max(MIN_STEP)
}

/// Centerline overlap count: how many mask-weighted dabs a point on the stroke receives. The
/// mask integral along the centerline is exactly `2 * size + 1` regardless of softness, so this
/// is the factor that turns a per-dab quantity into a per-unit-length one.
pub fn overlap(size: f32, step: f32) -> f32 {
    ((2.0 * size.max(0.0) + 1.0) / step.max(MIN_STEP)).max(1e-3)
}

/// Centerline integral of `mask²`. Unlike the plain mask integral this keeps a softness term, so
/// it is used to normalize the blur variance which accumulates `(sigma * mask)²` per dab.
pub fn mask_sq_integral(size: f32, softness: f32) -> f32 {
    let r = size.max(0.0);
    let s = softness.clamp(0.0, 1.0);
    2.0 * (1.0 - s) * r + 1.0 + (52.0 / 35.0) * s * r
}

/// Per-dab flow coefficient that yields `flow` interior opacity after [`overlap`] dabs. `flow` is
/// therefore independent of spacing, size and softness.
pub fn rate_coeff(flow: f32, overlap: f32) -> f32 {
    (1.0 - (1.0 - flow.clamp(0.0, 1.0)).powf(1.0 / overlap.max(1e-3))).clamp(0.0, 1.0)
}
