use glam::{Mat3, Vec4};

const OKLAB2LMS: Mat3 = Mat3::from_cols_array_2d(&[
    [1.0000000000, 1.0000000000, 1.0000000000],
    [0.3963377774, -0.1055613458, -0.0894841775],
    [0.2158037573, -0.0638541728, -1.2914855480],
]);

const LMS2RGB: Mat3 = Mat3::from_cols_array_2d(&[
    [4.0767416621, -1.2684380046, -0.0041960863],
    [-3.3077115913, 2.6097574011, -0.7034186147],
    [0.2309699292, -0.3413193965, 1.7076147010],
]);

const RGB2LMS: Mat3 = Mat3::from_cols_array_2d(&[
    [0.4122214708, 0.2119034982, 0.0883024619],
    [0.5363325363, 0.6806995451, 0.2817188376],
    [0.0514459929, 0.1073969566, 0.6299787005],
]);

const LMS2OKLAB: Mat3 = Mat3::from_cols_array_2d(&[
    [0.2104542553, 1.9779984951, 0.0259040371],
    [0.7936177850, -2.4285922050, 0.7827717662],
    [-0.0040720468, 0.4505937099, -0.8086757660],
]);

/// L_oklab -> L_srgb
pub fn oklab_to_linear_srgb(oklab: Vec4) -> Vec4 {
    let lms_ = OKLAB2LMS * oklab.truncate();
    let lms = lms_ * lms_ * lms_;
    (LMS2RGB * lms).extend(oklab.w)
}

/// L_oklab -> L_srgb
pub fn linear_srgb_to_oklab(rgba: Vec4) -> Vec4 {
    let lms = RGB2LMS * rgba.truncate();
    let lms_ = lms.powf(1. / 3.);
    (LMS2OKLAB * lms_).extend(rgba.w)
}
