//! Asystentka na pulpit — widget z planem dnia, licznikiem czasu i dziennikiem
//! pracy w Obsidianie. Specyfikacja: SPEC.md w katalogu głównym repozytorium.

mod aplikacja;
mod komendy;
mod obsidian;
mod okno;
mod system;
mod zasobnik;

pub mod czas;
pub mod dzien;
pub mod dziennik;
pub mod magazyn;
pub mod plik_zadan;
pub mod pliki;
pub mod rdzen;
pub mod sesja;
pub mod ustawienia;

use std::sync::Arc;

use tauri::{Manager, RunEvent, WindowEvent};
use tauri_plugin_global_shortcut::ShortcutState;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

use aplikacja::Aplikacja;

fn apl(app: &tauri::AppHandle) -> Option<Arc<Aplikacja>> {
    app.try_state::<Arc<Aplikacja>>().map(|s| Arc::clone(s.inner()))
}

pub fn run() {
    let poziom = if cfg!(debug_assertions) { log::LevelFilter::Debug } else { log::LevelFilter::Info };
    let aplikacja = tauri::Builder::default()
        // Drugie uruchomienie tylko pokazuje istniejące okno (B6).
        .plugin(tauri_plugin_single_instance::init(|app, argumenty, _katalog| {
            if let Some(apl) = apl(app) {
                // `asystentka --ustawienia` (np. ze skrótu) otwiera okno ustawień.
                if argumenty.iter().any(|a| a == "--ustawienia") {
                    apl.otworz_okno_ustawien();
                } else {
                    apl.wykonaj(|r, _| r.pokaz());
                }
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(poziom)
                .level_for("tao", log::LevelFilter::Warn)
                .level_for("wry", log::LevelFilter::Warn)
                .targets([
                    Target::new(TargetKind::Stdout),
                    Target::new(TargetKind::LogDir { file_name: Some("asystentka".into()) }),
                ])
                .max_file_size(1_000_000)
                .rotation_strategy(RotationStrategy::KeepOne)
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _skrot, zdarzenie| {
                    if zdarzenie.state == ShortcutState::Pressed {
                        if let Some(apl) = apl(app) {
                            apl.skrot();
                        }
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            komendy::migawka,
            komendy::gotowy,
            komendy::dopasuj_okno,
            komendy::rozpocznij,
            komendy::pauza,
            komendy::wznow,
            komendy::zrobione,
            komendy::przerwij,
            komendy::odpowiedz_jestes,
            komendy::wroc_do_listy,
            komendy::przelacz_wybor,
            komendy::zatwierdz_wybor,
            komendy::anuluj_wybor,
            komendy::zmien_plan,
            komendy::dodaj_zadanie,
            komendy::koniec_pracy,
            komendy::do_jutra,
            komendy::wroc_do_pracy,
            komendy::dalej_po_wczoraju,
            komendy::menu_kontekstowe,
            komendy::kreator_dane,
            komendy::kreator_zakoncz,
            komendy::ustawienia_pobierz,
            komendy::ustawienia_zapisz,
            komendy::otworz_ustawienia,
            komendy::plik_istnieje,
            komendy::ustaw_fokus,
            komendy::wybierz_plik,
            komendy::wybierz_folder,
        ])
        // Zdarzenia menu są globalne: menu pod prawym przyciskiem rozpoznajemy po prefiksie.
        .on_menu_event(|app, zdarzenie| {
            if zdarzenie.id().as_ref().starts_with(zasobnik::PREFIKS_KONTEKSTOWEGO) {
                if let Some(apl) = apl(app) {
                    apl.wybor_z_menu_kontekstowego(&zdarzenie);
                }
            }
        })
        .setup(|app| aplikacja::uruchom(app))
        .on_window_event(|okno, zdarzenie| match zdarzenie {
            WindowEvent::Moved(pozycja) if okno.label() == aplikacja::OKNO => {
                if let Some(apl) = apl(okno.app_handle()) {
                    apl.przesunieto(*pozycja);
                }
            }
            // Widget się nie zamyka (np. Alt+F4), tylko chowa; wyjście jest w zasobniku.
            WindowEvent::CloseRequested { api, .. } if okno.label() == aplikacja::OKNO => {
                api.prevent_close();
                if let Some(apl) = apl(okno.app_handle()) {
                    apl.wykonaj(|r, _| r.przelacz_widocznosc());
                }
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("nie udało się zbudować aplikacji");

    aplikacja.run(|app, zdarzenie| match zdarzenie {
        RunEvent::ExitRequested { api, code, .. } if code.is_none() => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(apl) = apl(app) {
                apl.zapisz_przy_wyjsciu();
            }
        }
        _ => {}
    });
}
