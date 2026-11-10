//! Testes do serviço com relógio manual. Cobrem os critérios de aceite da documentação (seção 9).

use std::sync::Arc;

use chrono::{NaiveDate, NaiveDateTime};
use tempo_core::{Clock, ClockTime, CoreError, ManualClock, Permille};
use tempo_store::{Error, NewReserved, NewTag, Tempo};

const BRT: i64 = -3 * 3600;
const H: i64 = 3600;
const M: i64 = 60;

fn local(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> NaiveDateTime {
    NaiveDate::from_ymd_opt(y, mo, d).unwrap().and_hms_opt(h, mi, 0).unwrap()
}

fn t(h: u8, m: u8) -> ClockTime {
    ClockTime::new(h, m).unwrap()
}

fn tag(name: &str, permille: u16) -> NewTag {
    NewTag { name: name.into(), color: 0x3B82F6, permille: Permille::new(permille).unwrap() }
}

fn block(name: &str, hours: i64) -> NewReserved {
    NewReserved { name: name.into(), duration: hours * H }
}

/// App com o bloco sugerido "Sono 8h" e dia começando às 07:00, às 09:00 de 09/11/2026.
fn setup(tags: &[NewTag]) -> (Tempo, Arc<ManualClock>) {
    let clock = Arc::new(ManualClock::at_local(local(2026, 11, 9, 9, 0), BRT));
    let mut app = Tempo::open_in_memory(clock.clone()).unwrap();
    app.prepare_onboarding().unwrap();
    app.complete_onboarding(t(7, 0), tags).unwrap();
    (app, clock)
}

fn reserved_id(app: &Tempo, name: &str) -> i64 {
    app.reserved().unwrap().into_iter().find(|r| r.name == name).unwrap().id
}

fn id_of(app: &Tempo, name: &str) -> i64 {
    app.tags(true).unwrap().into_iter().find(|t| t.name == name).unwrap().id
}

fn consumed(app: &mut Tempo, id: i64) -> i64 {
    app.tick().unwrap();
    app.today().unwrap().tags.into_iter().find(|t| t.tag.id == id).map_or(0, |t| t.consumed)
}

#[test]
fn calculo_basico_16h_e_10_por_cento() {
    let (mut app, _) = setup(&[tag("Estudo", 100)]);
    app.tick().unwrap();
    let v = app.today().unwrap();
    assert_eq!(v.available, 16 * H);
    assert_eq!(v.date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
    assert_eq!(v.day_elapsed, 2 * H);
    assert_eq!(v.day_remaining, 22 * H);
    assert_eq!(v.tags[0].allocated, H + 36 * M);
    assert_eq!(v.unallocated_permille.value(), 900);
    assert_eq!(v.unallocated_secs, 16 * H - (H + 36 * M));
}

#[test]
fn porcentagem_decimal() {
    let (mut app, _) = setup(&[]);
    let id = app.create_tag(&tag("Leitura", 123)).unwrap();
    app.tick().unwrap();
    let v = app.today().unwrap();
    assert_eq!(v.tags.iter().find(|x| x.tag.id == id).unwrap().allocated, 7_084);
}

#[test]
fn limite_de_100_por_cento() {
    let (mut app, _) = setup(&[tag("Trabalho", 950)]);
    let err = app.create_tag(&tag("Estudo", 100)).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::AllocationExceeded { .. })));
    assert_eq!(err.to_string(), "A soma das porcentagens ultrapassa 100%. Restam apenas 5%.");
    assert!(app.create_tag(&tag("Estudo", 50)).is_ok());
}

#[test]
fn nomes_unicos_sem_caixa_nem_acento() {
    let (mut app, _) = setup(&[tag("Exercício", 50)]);
    let err = app.create_tag(&tag("  exercicio ", 50)).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::NameDuplicate(_))));
}

#[test]
fn limite_de_20_tags() {
    let (mut app, _) = setup(&[]);
    for i in 0..20 {
        app.create_tag(&tag(&format!("Tag {i}"), 10)).unwrap();
    }
    assert!(matches!(app.create_tag(&tag("Tag 21", 10)), Err(Error::Rule(CoreError::TooManyTags))));
}

#[test]
fn um_timer_por_vez() {
    let (mut app, clock) = setup(&[tag("Estudo", 100), tag("Trabalho", 500)]);
    let estudo = id_of(&app, "Estudo");
    let trabalho = id_of(&app, "Trabalho");
    app.start(estudo).unwrap();
    clock.advance(10 * M);
    app.start(trabalho).unwrap();
    clock.advance(5 * M);
    assert_eq!(consumed(&mut app, estudo), 10 * M);
    assert_eq!(consumed(&mut app, trabalho), 5 * M);
    let v = app.today().unwrap();
    assert_eq!(v.active.as_ref().unwrap().tag_id, trabalho);
    assert!(v.tags.iter().find(|x| x.tag.id == trabalho).unwrap().running);
    assert!(!v.tags.iter().find(|x| x.tag.id == estudo).unwrap().running);
}

#[test]
fn pausar_retomar_e_parar() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(10 * M);
    assert!(app.pause().unwrap());
    clock.advance(30 * M);
    assert_eq!(consumed(&mut app, id), 10 * M);
    assert_eq!(app.today().unwrap().focus_tag, Some(id));
    app.start(id).unwrap();
    app.start(id).unwrap(); // idempotente
    clock.advance(5 * M);
    assert!(app.stop().unwrap());
    assert!(!app.stop().unwrap());
    assert_eq!(consumed(&mut app, id), 15 * M);
    let v = app.today().unwrap();
    assert_eq!(v.focus_tag, None);
    assert!(v.active.is_none());
    assert_eq!(app.sessions_today().unwrap().len(), 2);
}

#[test]
fn sobrevive_ao_encerramento_do_processo() {
    let path = std::env::temp_dir().join(format!("tempo-test-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let clock = Arc::new(ManualClock::at_local(local(2026, 11, 9, 9, 0), BRT));
    let id = {
        let mut app = Tempo::open(&path, clock.clone()).unwrap();
        app.prepare_onboarding().unwrap();
        app.complete_onboarding(t(7, 0), &[tag("Estudo", 100)]).unwrap();
        let id = id_of(&app, "Estudo");
        app.start(id).unwrap();
        clock.advance(10 * M);
        id
    }; // processo "morto"
    clock.advance(5 * M);
    let mut app = Tempo::open(&path, clock.clone()).unwrap();
    assert_eq!(consumed(&mut app, id), 15 * M);
    assert!(app.active_timer().unwrap().is_some());
    drop(app);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[test]
fn sem_excedente_para_no_instante_exato() {
    let (mut app, clock) = setup(&[tag("Exercício", 50)]); // 48min
    let id = id_of(&app, "Exercício");
    app.set_overtime(false).unwrap();
    app.start(id).unwrap();
    clock.advance(2 * H); // app ficou fechado bem depois do fim
    let r = app.tick().unwrap();
    assert_eq!(r.auto_stopped, Some(id));
    assert_eq!(consumed(&mut app, id), 48 * M);
    assert!(app.active_timer().unwrap().is_none());
    // Sem excedente, não dá para reiniciar uma tag esgotada.
    assert!(matches!(app.start(id), Err(Error::TimeExhausted(_))));
}

#[test]
fn com_excedente_continua_contando() {
    let (mut app, clock) = setup(&[tag("Exercício", 50)]);
    let id = id_of(&app, "Exercício");
    app.start(id).unwrap();
    clock.advance(60 * M);
    app.tick().unwrap();
    let v = app.today().unwrap();
    let t = v.tags.iter().find(|x| x.tag.id == id).unwrap();
    assert_eq!(t.progress().excess(), 12 * M);
    assert_eq!(t.progress().remaining(), 0);
    let a = v.active.unwrap();
    assert_eq!(a.deadline, a.start + 48 * M);
}

#[test]
fn desligar_excedente_durante_excedente_encerra_agora() {
    let (mut app, clock) = setup(&[tag("Exercício", 50)]);
    let id = id_of(&app, "Exercício");
    app.start(id).unwrap();
    clock.advance(60 * M);
    app.set_overtime(false).unwrap();
    clock.advance(10 * M);
    assert_eq!(consumed(&mut app, id), 60 * M);
    assert!(app.active_timer().unwrap().is_none());
}

#[test]
fn mudanca_no_tempo_reservado_vale_no_dia_seguinte() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let sono = reserved_id(&app, "Sono");
    app.update_reserved(sono, &block("Sono", 7)).unwrap();
    app.tick().unwrap();
    assert_eq!(app.today().unwrap().available, 16 * H);
    clock.set(clock.now() + 23 * H); // 10/11 08:00
    let r = app.tick().unwrap();
    assert!(r.day_rolled);
    let v = app.today().unwrap();
    assert_eq!(v.available, 17 * H);
    assert_eq!(v.date, NaiveDate::from_ymd_opt(2026, 11, 10).unwrap());
    // 10% de 17h = 1h42
    assert_eq!(v.tags[0].allocated, H + 42 * M);
}

#[test]
fn virada_do_dia_divide_timer_ativo() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    clock.set(clock.now() + 21 * H + 30 * M); // 10/11 06:30
    app.start(id).unwrap();
    clock.advance(H); // 07:30, passou da virada às 07:00
    app.tick().unwrap();
    let hist = app.history(10).unwrap();
    assert_eq!(hist.len(), 2);
    assert_eq!(hist[1].tags[0].consumed, 30 * M); // ontem: 06:30–07:00
    assert!(hist[1].closed);
    assert_eq!(consumed(&mut app, id), 30 * M); // hoje: 07:00–07:30
    assert!(app.active_timer().unwrap().is_some());
}

#[test]
fn dias_sem_uso_sao_criados_sem_registro() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    clock.advance(3 * 24 * H);
    app.tick().unwrap();
    let hist = app.history(10).unwrap();
    assert_eq!(hist.len(), 4);
    assert!(!hist[0].closed);
    assert!(hist[1].no_record && hist[2].no_record && hist[3].no_record);
    let dates: Vec<_> = hist.iter().map(|d| d.date.to_string()).collect();
    assert_eq!(dates, ["2026-11-12", "2026-11-11", "2026-11-10", "2026-11-09"]);
}

#[test]
fn ausencia_muito_longa_pula_direto_para_hoje() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    clock.advance(500 * 24 * H);
    app.tick().unwrap();
    assert_eq!(app.history(1000).unwrap().len(), 2);
}

#[test]
fn relogio_voltando_encerra_sessao() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(10 * M);
    app.tick().unwrap(); // grava last_seen
    clock.advance(-H);
    let r = app.tick().unwrap();
    assert!(r.clock_went_back);
    assert!(app.active_timer().unwrap().is_none());
    assert!(app.today().unwrap().clock_warning);
    assert!(consumed(&mut app, id) >= 0);
    app.dismiss_clock_warning().unwrap();
    assert!(!app.today().unwrap().clock_warning);
}

#[test]
fn lancamento_manual() {
    let (mut app, clock) = setup(&[tag("Estudo", 100), tag("Leitura", 100)]);
    let id = id_of(&app, "Estudo");
    let leitura = id_of(&app, "Leitura");
    let start = app.day_time_to_utc(t(7, 30)).unwrap();
    app.add_manual(id, start, 30 * M).unwrap();
    assert_eq!(consumed(&mut app, id), 30 * M);

    // Sobreposição com outra sessão (mesmo de outra tag)
    let err = app.add_manual(leitura, start + 10 * M, 10 * M).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::ManualOverlap)));
    // Encostar na sessão anterior é permitido
    app.add_manual(leitura, start + 30 * M, 10 * M).unwrap();
    // No futuro
    let err = app.add_manual(id, clock.now() - 5 * M, 10 * M).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::ManualInFuture)));
    // Antes do início do dia
    let err = app.add_manual(id, app.today().unwrap().day_start - H, 10 * M).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::ManualOutsideDay)));
    // Duração zero
    assert!(app.add_manual(id, start, 0).is_err());
    // Horário da madrugada pertence ao fim do dia lógico (dia seguinte no calendário)
    let late = app.day_time_to_utc(t(2, 0)).unwrap();
    assert!(late > app.today().unwrap().day_start + 18 * H);
}

#[test]
fn sobreposicao_com_timer_ativo() {
    let (mut app, clock) = setup(&[tag("Estudo", 100), tag("Leitura", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(30 * M);
    let err = app.add_manual(id_of(&app, "Leitura"), clock.now() - 20 * M, 10 * M).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::ManualOverlap)));
}

#[test]
fn apagar_sessao() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(10 * M);
    let active_id = app.active_timer().unwrap().unwrap().session_id;
    assert!(matches!(app.delete_session(active_id), Err(Error::SessionActive)));
    app.pause().unwrap();
    app.delete_session(active_id).unwrap();
    assert_eq!(consumed(&mut app, id), 0);
    assert!(matches!(app.delete_session(9999), Err(Error::SessionNotFound)));
}

#[test]
fn reduzir_alocacao_gera_excedente_e_preserva_consumido() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(H);
    app.pause().unwrap();
    app.update_tag(id, &tag("Estudo", 50)).unwrap(); // 48min
    app.tick().unwrap();
    let v = app.today().unwrap();
    let e = &v.tags[0];
    assert_eq!(e.allocated, 48 * M);
    assert_eq!(e.consumed, H);
    assert_eq!(e.progress().excess(), 12 * M);
}

#[test]
fn reduzir_alocacao_sem_excedente_com_timer_rodando_encerra_agora() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.set_overtime(false).unwrap();
    app.start(id).unwrap();
    clock.advance(H);
    app.update_tag(id, &tag("Estudo", 50)).unwrap();
    assert!(app.active_timer().unwrap().is_none());
    assert_eq!(consumed(&mut app, id), H);
}

#[test]
fn arquivar_restaurar_excluir() {
    let (mut app, clock) = setup(&[tag("Estudo", 400), tag("Trabalho", 500)]);
    let estudo = id_of(&app, "Estudo");
    app.start(estudo).unwrap();
    clock.advance(10 * M);
    app.archive_tag(estudo).unwrap();
    assert!(app.active_timer().unwrap().is_none());
    app.tick().unwrap();
    assert_eq!(app.today().unwrap().tags.len(), 1);
    assert_eq!(app.today().unwrap().allocated_permille.value(), 500);
    // Histórico de hoje mantém o consumido da tag arquivada
    let h = app.history(1).unwrap();
    assert_eq!(h[0].tags.iter().find(|x| x.tag_id == estudo).unwrap().consumed, 10 * M);

    // Sem espaço para restaurar
    let lazer = app.create_tag(&tag("Lazer", 200)).unwrap();
    assert!(matches!(app.restore_tag(estudo), Err(Error::NoRoomToRestore(_))));
    app.delete_tag(lazer).unwrap();
    app.restore_tag(estudo).unwrap();
    app.tick().unwrap();
    assert_eq!(app.today().unwrap().allocated_permille.value(), 900);

    app.delete_tag(estudo).unwrap();
    assert!(app.history(1).unwrap()[0].tags.iter().all(|x| x.tag_id != estudo));
    assert!(matches!(app.tag(estudo), Err(Error::TagNotFound)));
}

#[test]
fn reordenar_tags() {
    let (mut app, _) = setup(&[tag("A", 10), tag("B", 10), tag("C", 10)]);
    let c = id_of(&app, "C");
    app.move_tag(c, -1).unwrap();
    app.move_tag(c, -1).unwrap();
    app.move_tag(c, -1).unwrap(); // já é o primeiro
    let names: Vec<_> = app.tags(false).unwrap().into_iter().map(|t| t.name).collect();
    assert_eq!(names, ["C", "A", "B"]);
}

#[test]
fn estatisticas_dos_dias_fechados() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]); // 1h36 por dia
    let id = id_of(&app, "Estudo");
    // Dia 1: cumpre 100%
    app.add_manual(id, app.day_time_to_utc(t(7, 0)).unwrap(), 96 * M).unwrap();
    clock.advance(24 * H);
    app.tick().unwrap();
    // Dia 2: cumpre 50%
    app.add_manual(id, app.day_time_to_utc(t(7, 0)).unwrap(), 48 * M).unwrap();
    clock.advance(24 * H);
    app.tick().unwrap();
    let s = app.stats(7).unwrap();
    assert_eq!(s.days, 2);
    let e = &s.tags[0];
    assert_eq!(e.name, "Estudo");
    assert!((e.stats.compliance - 0.75).abs() < 1e-6);
    assert_eq!(e.stats.consumed_total, 144 * M);
    assert_eq!(e.stats.daily_average, 72 * M);
}

#[test]
fn exporta_csv() {
    let (mut app, _) = setup(&[tag("Estudo; casa", 100)]);
    let id = id_of(&app, "Estudo; casa");
    app.add_manual(id, app.day_time_to_utc(t(7, 30)).unwrap(), 30 * M).unwrap();
    let csv = app.export_csv().unwrap();
    let lines: Vec<_> = csv.lines().collect();
    assert_eq!(lines[0], "dia;tag;inicio;fim;duracao_segundos;duracao;origem");
    assert_eq!(lines[1], "2026-11-09;\"Estudo; casa\";2026-11-09 07:30:00;2026-11-09 08:00:00;1800;00:30:00;manual");
}

#[test]
fn retencao_apaga_dias_antigos() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    clock.advance(10 * 24 * H);
    app.tick().unwrap();
    assert_eq!(app.history(100).unwrap().len(), 11);
    app.set_retention(Some(5)).unwrap();
    assert_eq!(app.history(100).unwrap().len(), 6);
    assert_eq!(app.settings().unwrap().retention_days, Some(5));
}

#[test]
fn apagar_tudo_volta_ao_onboarding() {
    let (mut app, _) = setup(&[tag("Estudo", 100)]);
    app.erase_all().unwrap();
    let s = app.settings().unwrap();
    assert!(!s.onboarded);
    assert!(app.tags(true).unwrap().is_empty());
    assert!(matches!(app.today(), Err(Error::NotOnboarded)));
    assert!(!app.tick().unwrap().changed());
    assert!(app.reserved().unwrap().is_empty());
    app.prepare_onboarding().unwrap(); // volta a sugerir "Sono 8h"
    assert_eq!(app.reserved_total().unwrap(), 8 * H);
}

#[test]
fn excesso_de_compromisso_no_fim_do_dia() {
    let (mut app, clock) = setup(&[tag("Trabalho", 800)]); // 12h48
    app.tick().unwrap();
    assert!(!app.today().unwrap().overcommitted);
    clock.advance(11 * H); // 20:00 — restam 11h até o fim do dia, 12h48 por fazer
    app.tick().unwrap();
    assert!(app.today().unwrap().overcommitted);
}

#[test]
fn madrugada_antes_do_inicio_pertence_ao_dia_anterior() {
    let clock = Arc::new(ManualClock::at_local(local(2026, 11, 10, 2, 0), BRT));
    let mut app = Tempo::open_in_memory(clock.clone()).unwrap();
    app.prepare_onboarding().unwrap();
    let start = app.settings().unwrap().day_start; // padrão 04:00
    assert_eq!(start.to_string(), "04:00");
    app.complete_onboarding(start, &[]).unwrap();
    assert_eq!(app.today().unwrap().date, NaiveDate::from_ymd_opt(2026, 11, 9).unwrap());
    assert_eq!(app.today().unwrap().day_remaining, 2 * H);
}

#[test]
fn blocos_reservados_definem_o_tempo_disponivel() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    app.create_reserved(&block("Faculdade", 4)).unwrap();
    assert_eq!(app.reserved_total().unwrap(), 12 * H);
    assert_eq!(app.today().unwrap().available, 16 * H); // hoje não muda (RN16)
    clock.advance(24 * H);
    app.tick().unwrap();
    let v = app.today().unwrap();
    assert_eq!(v.available, 12 * H);
    assert_eq!(v.tags[0].allocated, H + 12 * M); // 10% de 12h
    let names: Vec<_> = app.reserved().unwrap().into_iter().map(|r| r.name).collect();
    assert_eq!(names, ["Sono", "Faculdade"]);
}

#[test]
fn crud_de_blocos_reservados() {
    let (mut app, _) = setup(&[]);
    // Nome único, sem diferenciar caixa e acento
    let err = app.create_reserved(&block("  sono ", 1)).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::NameDuplicate(_))));
    // Duração zero
    let err = app.create_reserved(&NewReserved { name: "X".into(), duration: 0 }).unwrap_err();
    assert!(matches!(err, Error::Rule(CoreError::DurationZero)));
    // Limite de 23h reservadas (8h + 15h ok, +1min não)
    let id = app.create_reserved(&block("Trabalho fixo", 15)).unwrap();
    let err = app.create_reserved(&NewReserved { name: "Y".into(), duration: 60 }).unwrap_err();
    assert_eq!(err.to_string(), "Os blocos reservados podem somar no máximo 23h. Restam apenas 0min para reservar.");
    // Editar considera os outros blocos
    app.update_reserved(id, &block("Trabalho", 10)).unwrap();
    assert_eq!(app.reserved_total().unwrap(), 18 * H);
    assert!(app.update_reserved(id, &block("Trabalho", 16)).is_err());
    // Excluir
    app.delete_reserved(id).unwrap();
    assert_eq!(app.reserved_total().unwrap(), 8 * H);
    assert!(matches!(app.delete_reserved(id), Err(Error::ReservedNotFound)));
    // Sem nenhum bloco, o dia inteiro fica disponível
    app.delete_reserved(reserved_id(&app, "Sono")).unwrap();
    assert_eq!(app.current_available().unwrap(), 16 * H); // dia atual mantém o snapshot
    assert_eq!(app.reserved_total().unwrap(), 0);
}

#[test]
fn converte_sono_da_versao_anterior() {
    let path = std::env::temp_dir().join(format!("tempo-legacy-{}.db", std::process::id()));
    let _ = std::fs::remove_file(&path);
    let clock = Arc::new(ManualClock::at_local(local(2026, 11, 9, 9, 0), BRT));
    drop(Tempo::open(&path, clock.clone()).unwrap());
    {
        let c = rusqlite::Connection::open(&path).unwrap();
        c.execute_batch("INSERT INTO settings VALUES ('bedtime', '23:30'), ('wake', '06:30'), ('onboarded', '1');")
            .unwrap();
    }
    let app = Tempo::open(&path, clock).unwrap();
    let r = app.reserved().unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!((r[0].name.as_str(), r[0].duration), ("Sono", 7 * H));
    assert_eq!(app.settings().unwrap().day_start.to_string(), "06:30");
    drop(app);
    for ext in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{ext}", path.display()));
    }
}

#[cfg(feature = "snapshot")]
#[test]
fn snapshot_ida_e_volta_mantem_timer_e_tags() {
    let (mut app, clock) = setup(&[tag("Estudo", 100)]);
    let id = id_of(&app, "Estudo");
    app.start(id).unwrap();
    clock.advance(10 * M);
    let bytes = app.snapshot().unwrap();
    drop(app);

    let mut back = Tempo::from_snapshot(&bytes, clock.clone()).unwrap();
    assert!(back.settings().unwrap().onboarded);
    assert_eq!(back.active_timer().unwrap().map(|a| a.tag_id), Some(id));
    assert_eq!(consumed(&mut back, id), 10 * M);

    let fresh = Tempo::from_snapshot(&[], clock).unwrap();
    assert!(!fresh.settings().unwrap().onboarded);
}
