pub const LIB_CAMERA: &'static str = include_str!("shaders/lib_camera.wgsl");
pub const LIB_COLORSPACE: &'static str = include_str!("shaders/lib_colorspace.wgsl");
pub const LIB_CONSTANT: &'static str = include_str!("shaders/lib_constant.wgsl");
pub const LIB_RECTANGLE: &'static str = include_str!("shaders/lib_rectangle.wgsl");

/// replace every `#key` into `value`
pub fn shader_compile(raw: &str, maps: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(raw.len() + 100);
    let mut pattern = String::new();
    let mut matching = false;

    let mut map = hashbrown::HashMap::new();
    map.insert("lib_camera", LIB_CAMERA);
    map.insert("lib_colorspace", LIB_COLORSPACE);
    map.insert("lib_constant", LIB_CONSTANT);
    map.insert("lib_rectangle", LIB_RECTANGLE);
    for &(key, value) in maps {
        map.insert(key, value);
    }

    for char in raw.chars() {
        if char == '#' {
            matching = true;
        } else if matching {
            if char.is_ascii_alphanumeric() || char == '_' {
                pattern.push(char);
            } else {
                if let Some(replacement) = map.get(&pattern[..]) {
                    result.push_str(replacement);
                } else {
                    result.push('#');
                    result.push_str(&pattern);
                }

                matching = false;
                pattern.clear();
                result.push(char);
            }
        } else {
            result.push(char);
        }
    }

    if matching {
        if let Some(replacement) = map.get(&pattern[..]) {
            result.push_str(replacement);
        } else {
            result.push('#');
            result.push_str(&pattern);
        }

        pattern.clear();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic() {
        assert_eq!(shader_compile("#foo", &[("foo", "bar")]), "bar");
    }

    #[test]
    fn test_separator() {
        assert_eq!(shader_compile("#foo baz", &[("foo", "bar")]), "bar baz");
    }

    #[test]
    fn test_trailing() {
        assert_eq!(shader_compile("end #foo", &[("foo", "bar")]), "end bar");
    }

    #[test]
    fn test_incomplete_match() {
        assert_eq!(shader_compile("#abc3", &[("abc", "value")]), "#abc3");
    }

    #[test]
    fn test_hash_without_key() {
        assert_eq!(shader_compile("# foo", &[]), "# foo");
    }

    #[test]
    fn test_unknown_key() {
        assert_eq!(shader_compile("#unkn own", &[]), "#unkn own");
    }
}
