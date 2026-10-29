//! Ikona w zasobniku i jej menu (SPEC.md, B6): Pokaż / Ukryj · Pauza / Wznów ·
//! Ukryj przed nagrywaniem · Ustawienia · Zakończ.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

pub const POKAZ: &str = "pokaz";
pub const PAUZA: &str = "pauza";
pub const NAGRYWANIE: &str = "nagrywanie";
pub const USTAWIENIA: &str = "ustawienia";
pub const ZAKONCZ: &str = "zakoncz";

/// Stan pozycji menu wyliczony z rdzenia.
#[derive(Debug, Clone, PartialEq)]
pub struct StanMenu {
    pub widoczna: bool,
    /// `None`: brak sesji; `Some(true)`: sesja zapauzowana.
    pub pauza: Option<bool>,
    pub ukryj_przed_nagrywaniem: bool,
}

pub struct Zasobnik {
    pokaz: MenuItem<Wry>,
    pauza: MenuItem<Wry>,
    nagrywanie: CheckMenuItem<Wry>,
    _ikona: TrayIcon<Wry>,
    ostatni: Mutex<Option<StanMenu>>,
}

impl Zasobnik {
    pub fn utworz(
        app: &AppHandle,
        stan: &StanMenu,
        menu_klik: impl Fn(&AppHandle, MenuEvent) + Send + Sync + 'static,
        ikona_klik: impl Fn(&AppHandle) + Send + Sync + 'static,
    ) -> tauri::Result<Zasobnik> {
        let pokaz = MenuItem::with_id(app, POKAZ, "Ukryj", true, None::<&str>)?;
        let pauza = MenuItem::with_id(app, PAUZA, "Pauza", false, None::<&str>)?;
        let nagrywanie = CheckMenuItem::with_id(
            app,
            NAGRYWANIE,
            "Ukryj przed nagrywaniem",
            true,
            stan.ukryj_przed_nagrywaniem,
            None::<&str>,
        )?;
        let ustawienia = MenuItem::with_id(app, USTAWIENIA, "Ustawienia", true, None::<&str>)?;
        let zakoncz = MenuItem::with_id(app, ZAKONCZ, "Zakończ", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &pokaz,
                &pauza,
                &PredefinedMenuItem::separator(app)?,
                &nagrywanie,
                &ustawienia,
                &PredefinedMenuItem::separator(app)?,
                &zakoncz,
            ],
        )?;
        let mut budowniczy = TrayIconBuilder::with_id("asystentka")
            .tooltip("Asystentka")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(menu_klik)
            .on_tray_icon_event(move |ikona, zdarzenie| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } =
                    zdarzenie
                {
                    ikona_klik(ikona.app_handle());
                }
            });
        if let Some(obraz) = app.default_window_icon() {
            budowniczy = budowniczy.icon(obraz.clone());
        }
        let ikona = budowniczy.build(app)?;
        let zasobnik = Zasobnik { pokaz, pauza, nagrywanie, _ikona: ikona, ostatni: Mutex::new(None) };
        zasobnik.odswiez(stan);
        Ok(zasobnik)
    }

    /// Zmienia tylko to, co się zmieniło. Nie trzyma blokady w czasie
    /// wywołań menu, które mogą czekać na wątek główny.
    pub fn odswiez(&self, stan: &StanMenu) {
        let poprzedni = {
            let mut ostatni = self.ostatni.lock().unwrap_or_else(|e| e.into_inner());
            if ostatni.as_ref() == Some(stan) {
                return;
            }
            ostatni.replace(stan.clone())
        };
        if poprzedni.as_ref().map(|p| p.widoczna) != Some(stan.widoczna) {
            let _ = self.pokaz.set_text(if stan.widoczna { "Ukryj" } else { "Pokaż" });
        }
        if poprzedni.as_ref().map(|p| p.pauza) != Some(stan.pauza) {
            let _ = self.pauza.set_text(if stan.pauza == Some(true) { "Wznów" } else { "Pauza" });
            let _ = self.pauza.set_enabled(stan.pauza.is_some());
        }
        if poprzedni.as_ref().map(|p| p.ukryj_przed_nagrywaniem) != Some(stan.ukryj_przed_nagrywaniem) {
            let _ = self.nagrywanie.set_checked(stan.ukryj_przed_nagrywaniem);
        }
    }

    pub fn nagrywanie_zaznaczone(&self) -> Option<bool> {
        self.nagrywanie.is_checked().ok()
    }
}
