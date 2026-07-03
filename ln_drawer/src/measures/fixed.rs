use glam::{DVec2, I64Vec2, IVec2, UVec2};

pub trait FI64Ext {
    type TyI32;
    type TyU32;
    type TyF64;

    fn q32_floor(self) -> Self::TyI32;
    fn q32_round(self) -> Self::TyI32;
    fn q32_ceil(self) -> Self::TyI32;

    fn q32_fract(self) -> Self::TyU32;

    fn q32_as_i32(self) -> Self::TyI32;
    fn q32_from_i32(val: Self::TyI32) -> Self;

    fn q32_as_f64(self) -> Self::TyF64;
    fn q32_from_f64(val: Self::TyF64) -> Self;
}

impl FI64Ext for i64 {
    type TyI32 = i32;
    type TyU32 = u32;
    type TyF64 = f64;

    #[inline]
    fn q32_floor(self) -> Self::TyI32 {
        (self >> 32) as i32
    }

    #[inline]
    fn q32_round(self) -> Self::TyI32 {
        ((self + (1i64 << 31)) >> 32) as i32
    }

    #[inline]
    fn q32_ceil(self) -> Self::TyI32 {
        (-((-self) >> 32)) as i32
    }

    #[inline]
    fn q32_fract(self) -> Self::TyU32 {
        self as u32
    }

    #[inline]
    fn q32_as_i32(self) -> Self::TyI32 {
        (self / (1i64 << 32)) as i32
    }

    #[inline]
    fn q32_from_i32(val: Self::TyI32) -> Self {
        val as i64 * (1i64 << 32)
    }

    #[inline]
    fn q32_as_f64(self) -> Self::TyF64 {
        self as f64 / (1u64 << 32) as f64
    }

    #[inline]
    fn q32_from_f64(val: Self::TyF64) -> Self {
        (val * (1u64 << 32) as f64) as i64
    }
}

impl FI64Ext for I64Vec2 {
    type TyI32 = IVec2;
    type TyU32 = UVec2;
    type TyF64 = DVec2;

    #[inline]
    fn q32_floor(self) -> Self::TyI32 {
        (self >> 32i32).as_ivec2()
    }

    #[inline]
    fn q32_round(self) -> Self::TyI32 {
        ((self + (1i64 << 31)) >> 32i32).as_ivec2()
    }

    #[inline]
    fn q32_ceil(self) -> Self::TyI32 {
        (-((-self) >> 32i32)).as_ivec2()
    }

    #[inline]
    fn q32_fract(self) -> Self::TyU32 {
        self.as_uvec2()
    }

    #[inline]
    fn q32_as_i32(self) -> Self::TyI32 {
        (self / (1i64 << 32)).as_ivec2()
    }

    #[inline]
    fn q32_from_i32(val: Self::TyI32) -> Self {
        val.as_i64vec2() * (1i64 << 32)
    }

    #[inline]
    fn q32_as_f64(self) -> Self::TyF64 {
        self.as_dvec2() / (1u64 << 32) as f64
    }

    #[inline]
    fn q32_from_f64(val: Self::TyF64) -> Self {
        (val * (1u64 << 32) as f64).as_i64vec2()
    }
}
