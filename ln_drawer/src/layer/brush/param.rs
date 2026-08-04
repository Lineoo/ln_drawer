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
            inner: ParamCurve::Constant { val },
            scale: 1.0,
            offset: 0.0,
        }
    }

    pub const fn force_index(min: f32, max: f32, idx: f32) -> BrushParam<f32> {
        BrushParam {
            inner: ParamCurve::ForceIndex { min, max, idx },
            scale: 1.0,
            offset: 0.0,
        }
    }

    pub fn get(&self, draw: Draw) -> f32 {
        match self.inner {
            ParamCurve::Constant { val: value } => value,
            ParamCurve::ForceIndex { min, max, idx } => min + (max - min) * draw.force.powf(idx),
        }
    }
}
