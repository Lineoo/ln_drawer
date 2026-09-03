static TRF: std::sync::LazyLock<toml::Value> = std::sync::LazyLock::new(|| {
    let locale = sys_locale::get_locale().unwrap_or_default();
    let locale = icu::locale::Locale::try_from_str(&locale).unwrap();

    let fallbacker = icu::locale::fallback::LocaleFallbacker::new();
    let mut iter = fallbacker
        .for_config(Default::default())
        .fallback_for(locale.id.into());

    let table = loop {
        let label = iter.get().to_string();
        log::debug!("match {label}");
        match &label[..] {
            "zh-CN" => break include_str!("../lang/zh-CN.toml"),
            "zh-Hant" => break include_str!("../lang/zh-Hant.toml"),
            "zh" => break include_str!("../lang/zh-CN.toml"),
            "en" => break include_str!("../lang/en.toml"),
            "jp" => break include_str!("../lang/jp.toml"),
            "ar" => break include_str!("../lang/ar.toml"),
            "und" => break include_str!("../lang/en.toml"),
            _ => iter.step(),
        };
    };

    toml::from_str(table).unwrap()
});

static FALLBACK_TRF: std::sync::LazyLock<toml::Value> =
    std::sync::LazyLock::new(|| toml::from_str(include_str!("../lang/en.toml")).unwrap());

pub fn tr(key: &str) -> &str {
    let mut rf = &*TRF;
    for i in key.split('.') {
        if let Some(nxt) = rf.get(i) {
            rf = nxt
        } else {
            break;
        }
    }
    if let Some(rst) = rf.as_str() {
        rst
    } else {
        rf = &*FALLBACK_TRF;
        for i in key.split('.') {
            if let Some(nxt) = rf.get(i) {
                rf = nxt
            } else {
                break;
            }
        }
        if let Some(rst) = rf.as_str() {
            rst
        } else {
            key
        }
    }
}

pub fn trp(key: &str, params: &[(&str, &str)]) -> String {
    let mut rf = &*TRF;
    for i in key.split('.') {
        if let Some(nxt) = rf.get(i) {
            rf = nxt
        } else {
            break;
        }
    }
    if let Some(rst) = rf.as_str() {
        compile(rst, params)
    } else {
        rf = &*FALLBACK_TRF;
        for i in key.split('.') {
            if let Some(nxt) = rf.get(i) {
                rf = nxt
            } else {
                break;
            }
        }
        if let Some(rst) = rf.as_str() {
            compile(rst, params)
        } else {
            compile(key, params)
        }
    }
}

fn compile(raw: &str, maps: &[(&str, &str)]) -> String {
    let mut result = String::with_capacity(raw.len() + 100);
    let mut pattern = String::new();
    let mut matching = 0;

    let mut map = hashbrown::HashMap::new();
    for &(key, value) in maps {
        map.insert(key, value);
    }

    for ch in raw.chars() {
        matching = match (matching, ch) {
            (0, '{') => 1,
            (0, '}') => -1,
            (0, _) => {
                result.push(ch);
                0
            }
            (1.., '{') => {
                result.push('{');
                0
            }
            (1.., '}') => {
                if let Some(v) = map.get(&pattern[..]) {
                    result.push_str(*v);
                }
                pattern.clear();
                0
            }
            (1.., _) => {
                pattern.push(ch);
                1
            }
            (..0, '{') => {
                result.push('}');
                result.push('{');
                0
            }
            (..0, '}') => {
                result.push('}');
                0
            }
            (..0, _) => {
                result.push('}');
                result.push(ch);
                0
            }
        };
    }

    result
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn param_compile() {
        assert_eq!(compile("Hi {name}", &[("name", "John")]), "Hi John");
        assert_eq!(compile("Hi {{name}}", &[("name", "John")]), "Hi {name}");
        assert_eq!(
            compile("Hi {name} there", &[("name", "John")]),
            "Hi John there"
        );
        assert_eq!(
            compile("Hi {{name}} there", &[("name", "John")]),
            "Hi {name} there"
        );
        assert_eq!(compile("Hi {name", &[("name", "John")]), "Hi ");
        assert_eq!(compile("Hi {{name", &[("name", "John")]), "Hi {name");
        assert_eq!(compile("Hi {name there", &[("name", "John")]), "Hi ");
        assert_eq!(
            compile("Hi {{name there", &[("name", "John")]),
            "Hi {name there"
        );

        assert_eq!(
            compile(
                "{greeting} {name}",
                &[("name", "John"), ("greeting", "Hello")]
            ),
            "Hello John"
        );
        assert_eq!(
            compile(
                "{greeting} {und}",
                &[("name", "John"), ("greeting", "Hello")]
            ),
            "Hello "
        );
    }
}
