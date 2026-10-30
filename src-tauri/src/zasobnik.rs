//! Menu zasobnika i menu pod prawym przyciskiem (SPEC.md, B6).
//!
//! Na Windows 11 ikona zasobnika domyślnie trafia pod strzałkę „^”, dlatego
//! wszystkie polecenia są też w menu pod prawym przyciskiem na postaci i panelu.
//!
//! Zasobnik: Pokaż / Ukryj · Pauza / Wznów · Koniec pracy na dziś · Ukryj przed
//! nagrywaniem · Ustawienia · Zamknij asystentkę. Lewy klik w ikonę pokazuje i
//! chowa widget.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::rdzen::Mozliwosci;

pub const POKAZ: &str = "pokaz";
pub const PAUZA: &str = "pauza";
pub const KONIEC: &str = "koniec";
pub const NAGRYWANIE: &str = "nagrywanie";
pub const USTAWIENIA: &str = "ustawienia";
pub const ZAMKNIJ: &str = "zamknij";

/// Identyfikatory menu pod prawym przyciskiem mają wspólny prefiks, żeby
/// odróżnić je od menu zasobnika (zdarzenia menu są globalne).
pub const PREFIKS_KONTEKSTOWEGO: &str = "ctx_";
pub const CTX_KONIEC: &str = "ctx_koniec";
pub const CTX_PLAN: &str = "ctx_plan";
pub const CTX_DODAJ: &str = "ctx_dodaj";
pub const CTX_PAUZA: &str = "ctx_pauza";
pub const CTX_UKRYJ: &str = "ctx_ukryj";
pub const CTX_USTAWIENIA: &str = "ctx_ustawienia";
pub const CTX_ZAMKNIJ: &str = "ctx_zamknij";

/// Stan pozycji menu zasobnika wyliczony z rdzenia.
#[derive(Debug, Clone, PartialEq)]
pub struct StanMenu {
    pub widoczna: bool,
    /// `None`: brak sesji; `Some(true)`: sesja zapauzowana.
    pub pauza: Option<bool>,
    pub koniec_pracy: bool,
    pub ukryj_przed_nagrywaniem: bool,
}

impl StanMenu {
    pub fn z(m: &Mozliwosci, ukryj_przed_nagrywaniem: bool) -> StanMenu {
        StanMenu { widoczna: m.widoczna, pauza: m.pauza, koniec_pracy: m.koniec_pracy, ukryj_przed_nagrywaniem }
    }
}

pub struct Zasobnik {
    pokaz: MenuItem<Wry>,
    pauza: MenuItem<Wry>,
    koniec: MenuItem<Wry>,
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
        let koniec = MenuItem::with_id(app, KONIEC, "Koniec pracy na dziś", stan.koniec_pracy, None::<&str>)?;
        let nagrywanie = CheckMenuItem::with_id(
            app,
            NAGRYWANIE,
            "Ukryj przed nagrywaniem",
            true,
            stan.ukryj_przed_nagrywaniem,
            None::<&str>,
        )?;
        let ustawienia = MenuItem::with_id(app, USTAWIENIA, "Ustawienia", true, None::<&str>)?;
        let zamknij = MenuItem::with_id(app, ZAMKNIJ, "Zamknij asystentkę", true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &pokaz,
                &pauza,
                &koniec,
                &PredefinedMenuItem::separator(app)?,
                &nagrywanie,
                &ustawienia,
                &PredefinedMenuItem::separator(app)?,
                &zamknij,
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
        let zasobnik = Zasobnik { pokaz, pauza, koniec, nagrywanie, _ikona: ikona, ostatni: Mutex::new(None) };
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
        if poprzedni.as_ref().map(|p| p.koniec_pracy) != Some(stan.koniec_pracy) {
            let _ = self.koniec.set_enabled(stan.koniec_pracy);
        }
        if poprzedni.as_ref().map(|p| p.ukryj_przed_nagrywaniem) != Some(stan.ukryj_przed_nagrywaniem) {
            let _ = self.nagrywanie.set_checked(stan.ukryj_przed_nagrywaniem);
        }
    }

    pub fn nagrywanie_zaznaczone(&self) -> Option<bool> {
        self.nagrywanie.is_checked().ok()
    }
}

/// Menu pod prawym przyciskiem na postaci i panelu: Koniec pracy na dziś ·
/// Zmień plan · Dodaj zadanie · Pauza / Wznów · Ukryj · Ustawienia · Zamknij
/// asystentkę. Pozycje, których w danym stanie nie ma sensu używać, są wyszarzone.
pub fn menu_kontekstowe(app: &AppHandle, m: &Mozliwosci) -> tauri::Result<Menu<Wry>> {
    let koniec = MenuItem::with_id(app, CTX_KONIEC, "Koniec pracy na dziś", m.koniec_pracy, None::<&str>)?;
    let plan = MenuItem::with_id(app, CTX_PLAN, "Zmień plan", m.zmien_plan, None::<&str>)?;
    let dodaj = MenuItem::with_id(app, CTX_DODAJ, "Dodaj zadanie", m.dodaj_zadanie, None::<&str>)?;
    let pauza = MenuItem::with_id(
        app,
        CTX_PAUZA,
        if m.pauza == Some(true) { "Wznów" } else { "Pauza" },
        m.pauza.is_some(),
        None::<&str>,
    )?;
    let ukryj = MenuItem::with_id(app, CTX_UKRYJ, "Ukryj", true, None::<&str>)?;
    let ustawienia = MenuItem::with_id(app, CTX_USTAWIENIA, "Ustawienia", true, None::<&str>)?;
    let zamknij = MenuItem::with_id(app, CTX_ZAMKNIJ, "Zamknij asystentkę", true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &koniec,
            &plan,
            &dodaj,
            &PredefinedMenuItem::separator(app)?,
            &pauza,
            &ukryj,
            &PredefinedMenuItem::separator(app)?,
            &ustawienia,
            &zamknij,
        ],
    )
}
