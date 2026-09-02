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

pub fn tr(key: &str) -> String {
    let mut rf = &*TRF;
    for i in key.split('.') {
        if let Some(nxt) = rf.get(i) {
            rf = nxt
        } else {
            return key.into();
        }
    }
    if let Some(rst) = rf.as_str() {
        rst.into()
    } else {
        key.into()
    }
}
