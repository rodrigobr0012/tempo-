//! Integração com o Android via JNI (RNF03).
//!
//! - `android_main`: ponto de entrada chamado pela `NativeActivity`.
//! - [`AndroidPlatform`]: chama a classe Java `com.projetotempo.Bridge` (notificação, alarme,
//!   permissões, compartilhamento). A Bridge não tem regra de negócio: só encapsula APIs do Android.
//! - `Java_com_projetotempo_Native_onCommand`: chamado pelos receptores Java (botões da
//!   notificação, alarme de fim, boot, mudança de relógio). Aplica o comando no serviço Rust
//!   e devolve ao sistema o estado atualizado do timer.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use jni::objects::{JClass, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use slint::android::AndroidApp;

use crate::platform::{timer_sync, Platform, TimerSync};
use crate::state;

const BRIDGE: &str = "com.projetotempo.Bridge";
const DB_NAME: &str = "tempo.db";

fn init_logger() {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("tempo"),
    );
}

#[no_mangle]
fn android_main(app: AndroidApp) {
    init_logger();
    let dir = app.internal_data_path().unwrap_or_else(|| PathBuf::from("/data/local/tmp"));
    if let Err(e) = slint::android::init(app.clone()) {
        log::error!("falha ao iniciar o Slint: {e}");
        return;
    }
    let platform = Rc::new(AndroidPlatform { app });
    if let Err(e) = crate::run(dir.join(DB_NAME), platform) {
        log::error!("erro na interface: {e}");
    }
}

/// Carrega a classe Bridge pelo class loader do app (a thread nativa não enxerga as classes do app).
fn bridge_class<'a>(env: &mut JNIEnv<'a>, ctx: &JObject) -> jni::errors::Result<JClass<'a>> {
    let loader = env.call_method(ctx, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    let name = env.new_string(BRIDGE)?;
    let class = env
        .call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?
        .l()?;
    Ok(JClass::from(class))
}

fn clear_exception(env: &mut JNIEnv) {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
}

/// Mostra/limpa a notificação do timer e o alarme de fim.
fn sync_timer_jni(env: &mut JNIEnv, ctx: &JObject, timer: Option<&TimerSync>) -> jni::errors::Result<()> {
    let class = bridge_class(env, ctx)?;
    match timer {
        Some(t) => {
            let name = env.new_string(&t.tag_name)?;
            env.call_static_method(
                &class,
                "showTimer",
                "(Landroid/content/Context;Ljava/lang/String;IJJZZZ)V",
                &[
                    JValue::Object(ctx),
                    JValue::Object(&name),
                    JValue::Int(t.color as i32),
                    JValue::Long(t.start_ms),
                    JValue::Long(t.deadline_ms),
                    JValue::Bool(t.overtime.into()),
                    JValue::Bool(t.sound.into()),
                    JValue::Bool(t.vibration.into()),
                ],
            )?;
        }
        None => {
            env.call_static_method(&class, "clearTimer", "(Landroid/content/Context;)V", &[JValue::Object(ctx)])?;
        }
    }
    Ok(())
}

pub struct AndroidPlatform {
    app: AndroidApp,
}

impl AndroidPlatform {
    fn with_env<R>(&self, f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<R>) -> Option<R> {
        // SAFETY: os ponteiros vêm do android-activity e valem enquanto o app existir.
        let vm = unsafe { JavaVM::from_raw(self.app.vm_as_ptr() as *mut jni::sys::JavaVM) }.ok()?;
        let mut env = vm.attach_current_thread_permanently().ok()?;
        let activity = unsafe { JObject::from_raw(self.app.activity_as_ptr() as jni::sys::jobject) };
        let result = env.with_local_frame(32, |env| f(env, &activity));
        clear_exception(&mut env);
        match result {
            Ok(v) => Some(v),
            Err(e) => {
                log::error!("JNI: {e}");
                None
            }
        }
    }

    fn call_bool(&self, method: &str) -> bool {
        self.with_env(|env, ctx| {
            let class = bridge_class(env, ctx)?;
            env.call_static_method(&class, method, "(Landroid/content/Context;)Z", &[JValue::Object(ctx)])?.z()
        })
        .unwrap_or(false)
    }

    fn call_void(&self, method: &str) {
        self.with_env(|env, ctx| {
            let class = bridge_class(env, ctx)?;
            env.call_static_method(&class, method, "(Landroid/app/Activity;)V", &[JValue::Object(ctx)])?;
            Ok(())
        });
    }
}

impl Platform for AndroidPlatform {
    fn is_android(&self) -> bool {
        true
    }

    fn sync_timer(&self, timer: Option<&TimerSync>) {
        self.with_env(|env, ctx| sync_timer_jni(env, ctx, timer));
    }

    fn notify_finished(&self, tag_name: &str, sound: bool, vibration: bool) {
        self.with_env(|env, ctx| {
            let class = bridge_class(env, ctx)?;
            let name = env.new_string(tag_name)?;
            env.call_static_method(
                &class,
                "notifyFinished",
                "(Landroid/content/Context;Ljava/lang/String;ZZ)V",
                &[
                    JValue::Object(ctx),
                    JValue::Object(&name),
                    JValue::Bool(sound.into()),
                    JValue::Bool(vibration.into()),
                ],
            )?;
            Ok(())
        });
    }

    fn notifications_allowed(&self) -> bool {
        self.call_bool("notificationsAllowed")
    }

    fn request_notifications(&self) {
        self.call_void("requestNotifications");
    }

    fn exact_alarms_allowed(&self) -> bool {
        self.call_bool("exactAlarmsAllowed")
    }

    fn request_exact_alarms(&self) {
        self.call_void("requestExactAlarms");
    }

    fn set_dark_bars(&self, dark: bool) {
        self.with_env(|env, ctx| {
            let class = bridge_class(env, ctx)?;
            env.call_static_method(
                &class,
                "setDarkBars",
                "(Landroid/app/Activity;Z)V",
                &[JValue::Object(ctx), JValue::Bool(dark.into())],
            )?;
            Ok(())
        });
    }

    fn share_text(&self, title: &str, text: &str) -> Result<String, String> {
        self.with_env(|env, ctx| {
            let class = bridge_class(env, ctx)?;
            let t = env.new_string(title)?;
            let body = env.new_string(text)?;
            env.call_static_method(
                &class,
                "shareText",
                "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)V",
                &[JValue::Object(ctx), JValue::Object(&t), JValue::Object(&body)],
            )?;
            Ok(())
        })
        .map(|_| String::new())
        .ok_or_else(|| "Não foi possível abrir o compartilhamento.".to_string())
    }
}

/// Comandos vindos dos receptores Java: "pause", "stop", "deadline", "sync".
fn handle_command(env: &mut JNIEnv, ctx: &JObject, files_dir: &Path, command: &str) {
    if let Err(e) = state::init(&files_dir.join(DB_NAME)) {
        log::error!("banco: {e}");
        return;
    }
    let result = state::with(|t| {
        t.tick()?;
        match command {
            "pause" => {
                t.pause()?;
            }
            "stop" => {
                t.stop()?;
            }
            _ => {} // "deadline", "sync": o tick já aplicou as regras (RN20, RN21, RN24)
        }
        timer_sync(t)
    });
    match result {
        Ok(sync) => {
            if let Err(e) = sync_timer_jni(env, ctx, sync.as_ref()) {
                log::error!("JNI: {e}");
            }
            clear_exception(env);
        }
        Err(tempo_store::Error::NotOnboarded) => {}
        Err(e) => log::error!("comando {command}: {e}"),
    }
}

#[no_mangle]
pub extern "system" fn Java_com_projetotempo_Native_onCommand<'l>(
    mut env: JNIEnv<'l>,
    _class: JClass<'l>,
    context: JObject<'l>,
    files_dir: JString<'l>,
    command: JString<'l>,
) {
    init_logger();
    // Nunca deixar um pânico atravessar a fronteira JNI.
    let _ = catch_unwind(AssertUnwindSafe(|| {
        let dir: String = match env.get_string(&files_dir) {
            Ok(s) => s.into(),
            Err(_) => return,
        };
        let cmd: String = match env.get_string(&command) {
            Ok(s) => s.into(),
            Err(_) => return,
        };
        handle_command(&mut env, &context, Path::new(&dir), &cmd);
    }));
}
