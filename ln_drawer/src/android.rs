use std::sync::OnceLock;

use android_logger::{Config, FilterBuilder};
use jni::{
    EnvUnowned,
    objects::{JObject, JString},
    sys::jobject,
};
use winit::platform::android::activity::AndroidApp;

static FILES_DIR: OnceLock<String> = OnceLock::new();

pub fn init_logging() {
    android_logger::init_once(
        Config::default()
            .with_max_level(log::LevelFilter::Debug)
            .with_filter(
                FilterBuilder::new()
                    .filter(None, log::LevelFilter::Debug)
                    .filter(Some("naga"), log::LevelFilter::Warn)
                    .filter(Some("wgpu"), log::LevelFilter::Warn)
                    .build(),
            )
            .with_tag("ln_drawer"),
    );
}

pub fn notify_java(app: &AndroidApp, message: &str) {
    let app_for_java = app.clone();
    let message = message.to_owned();

    app.run_on_java_main_thread(Box::new(move || {
        let Ok(vm) = jni::JavaVM::singleton() else {
            return;
        };

        let result = vm.attach_current_thread(|env| -> jni::errors::Result<()> {
            let raw = app_for_java.activity_as_ptr() as jobject;
            let activity = unsafe { env.as_cast_raw::<JObject<'_>>(&raw)? };
            let message = JString::new(env, &message)?;

            env.call_method(
                activity.as_ref(),
                jni::jni_str!("onNativeReady"),
                jni::jni_sig!("(Ljava/lang/String;)V"),
                &[jni::JValue::Object(message.as_ref())],
            )?;

            Ok(())
        });

        if let Err(err) = result {
            log::error!("failed to call Kotlin onNativeReady: {err:?}");
        }
    }));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_linn_lndrawer_MainActivity_nativeInit<'local>(
    mut env: EnvUnowned<'local>,
    _this: JObject<'local>,
    files_dir: JString<'local>,
) -> JString<'local> {
    init_logging();

    env.with_env(|env| -> Result<JString<'local>, jni::errors::Error> {
        let files_dir = files_dir.to_string();
        log::info!("JNI Kotlin -> Rust: filesDir={files_dir}");
        let _ = FILES_DIR.set(files_dir);
        JString::new(env, "pong from Rust")
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}
