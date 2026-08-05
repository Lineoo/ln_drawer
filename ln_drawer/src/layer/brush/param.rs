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
