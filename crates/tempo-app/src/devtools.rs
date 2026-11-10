//! Modo de capturas de tela para desenvolvimento: `TEMPO_SNAPSHOTS=<pasta> tempo-desktop`.
//! Cria um banco de demonstração, percorre as telas e salva um PNG de cada uma.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use slint::{ComponentHandle, Timer, TimerMode};
use tempo_core::{ClockTime, Permille, SleepSchedule};
use tempo_store::NewTag;

use crate::platform::DesktopPlatform;
use crate::{state, ui, Api, AppWindow, Dlg};

type Step = (&'static str, fn(&AppWindow));

const STEPS: &[Step] = &[
    ("01-boas-vindas", |_| {}),
    ("02-hoje", |w| {
        seed();
        let api = w.global::<Api>();
        api.set_onboarded(true);
        api.invoke_navigate(0);
    }),
    ("03-planejar", |w| w.global::<Api>().invoke_navigate(1)),
    ("04-editor", |w| {
        let d = w.global::<Dlg>();
        let tag = w.global::<Api>().get_plan_tags();
        if let Some(t) = slint::Model::row_data(&tag, 1) {
            d.set_editor_id(t.id);
            d.set_editor_name(t.name);
            d.set_editor_color(t.color);
            d.set_editor_permille(t.permille);
            d.set_editor_max(w.global::<Api>().invoke_max_permille_for(t.id));
        }
        d.set_kind(1);
    }),
    ("05-historico", |w| {
        w.global::<Dlg>().set_kind(0);
        w.global::<Api>().invoke_navigate(2);
    }),
    ("06-estatisticas", |w| {
        let api = w.global::<Api>();
        api.set_history_tab(1);
        api.invoke_load_history(1);
    }),
    ("07-ajustes", |w| w.global::<Api>().invoke_navigate(3)),
    ("08-lancar", |w| {
        w.global::<Api>().invoke_navigate(0);
        let d = w.global::<Dlg>();
        d.set_manual_start(w.global::<Api>().get_manual_default_start());
        d.set_manual_minutes(30);
        d.set_kind(2);
    }),
    ("09-sessoes", |w| {
        w.global::<Api>().invoke_open_sessions();
        w.global::<Dlg>().set_kind(3);
    }),
    ("10-confirmar", |w| {
        let d = w.global::<Dlg>();
        d.set_confirm_title("Apagar todos os dados?".into());
        d.set_confirm_text("Tags, sessões, histórico e ajustes serão apagados. Isso não pode ser desfeito.".into());
        d.set_confirm_button("Apagar tudo".into());
        d.set_kind(4);
    }),
];

/// Dados de demonstração: sessões encerradas e o timer de "Estudo" rodando.
fn seed() {
    let _ = state::with(|t| {
        t.complete_onboarding(
            SleepSchedule::new(ClockTime::new(23, 0)?, ClockTime::new(7, 0)?)?,
            &[
                NewTag { name: "Trabalho".into(), color: 0x3B82F6, permille: Permille::new(500)? },
                NewTag { name: "Estudo".into(), color: 0x8B5CF6, permille: Permille::new(100)? },
                NewTag { name: "Exercício".into(), color: 0x10B981, permille: Permille::new(50)? },
                NewTag { name: "Leitura".into(), color: 0xF59E0B, permille: Permille::new(75)? },
            ],
        )?;
        let tags = t.tags(false)?;
        let now = t.now();
        let day_start = t.today()?.day_start;
        // Até 3h de trabalho e 50min de exercício antes de agora, se couber no dia.
        let room = now - day_start;
        if room > 4 * 3600 {
            let _ = t.add_manual(tags[0].id, now - 4 * 3600, 3 * 3600);
            let _ = t.add_manual(tags[2].id, now - 3600, 50 * 60);
        }
        t.start(tags[1].id)?;
        Ok(())
    });
}

fn save_png(window: &AppWindow, path: &Path) -> Result<(), String> {
    let buf = window.window().take_snapshot().map_err(|e| e.to_string())?;
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), buf.width(), buf.height());
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(buf.as_bytes()).map_err(|e| e.to_string())
}

pub fn snapshots(out: PathBuf) -> Result<(), slint::PlatformError> {
    std::fs::create_dir_all(&out).map_err(|e| slint::PlatformError::Other(e.to_string()))?;
    let db = out.join("demo.db");
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", db.display()));
    }
    state::init(&db).map_err(|e| slint::PlatformError::Other(e.to_string()))?;

    let window = AppWindow::new()?;
    window.window().set_size(slint::LogicalSize::new(400.0, 820.0));
    let handle = ui::setup(&window, Rc::new(DesktopPlatform { data_dir: out.clone() }));

    let index = Rc::new(Cell::new(0usize));
    let timer = Timer::default();
    let weak = window.as_weak();
    timer.start(TimerMode::Repeated, Duration::from_millis(900), move || {
        let Some(w) = weak.upgrade() else { return };
        let i = index.get();
        if i > 0 {
            let (name, _) = STEPS[i - 1];
            let path = out.join(format!("{name}.png"));
            match save_png(&w, &path) {
                Ok(()) => println!("{}", path.display()),
                Err(e) => eprintln!("{name}: {e}"),
            }
        }
        if i == STEPS.len() {
            let _ = slint::quit_event_loop();
            return;
        }
        (STEPS[i].1)(&w);
        index.set(i + 1);
    });
    window.run()?;
    drop(handle);
    Ok(())
}
