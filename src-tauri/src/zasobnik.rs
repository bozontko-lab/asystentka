//! Menu zasobnika i menu pod prawym przyciskiem (SPEC.md, B6).
//!
//! Na Windows 11 ikona zasobnika domyślnie trafia pod strzałkę „^”, dlatego
//! wszystkie polecenia są też w menu pod prawym przyciskiem na postaci i panelu.
//!
//! Zasobnik: Pokaż / Ukryj · Pauza / Wznów · Koniec pracy na dziś · Przywróć
//! pozycję · Ukryj przed nagrywaniem · Dźwięk 40 Hz · Ustawienia · Zamknij {postac}. Lewy klik w
//! ikonę pokazuje i chowa widget; podpowiedź ikony to imię postaci.

use std::sync::Mutex;

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::rdzen::Mozliwosci;

pub const POKAZ: &str = "pokaz";
pub const PAUZA: &str = "pauza";
pub const KONIEC: &str = "koniec";
pub const PRZYWROC: &str = "przywroc";
pub const NAGRYWANIE: &str = "nagrywanie";
pub const DZWIEK: &str = "dzwiek";
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
pub const CTX_PRZYWROC: &str = "ctx_przywroc";
pub const CTX_DZWIEK: &str = "ctx_dzwiek";
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
    /// Przełącznik „Dźwięk 40 Hz” (B7): zaznaczony, gdy dźwięk skupienia jest włączony.
    pub dzwiek: bool,
    /// Imię postaci: w „Zamknij {postac}” i w podpowiedzi ikony (B6).
    pub postac: String,
}

impl StanMenu {
    pub fn z(m: &Mozliwosci, ukryj_przed_nagrywaniem: bool, dzwiek: bool, postac: &str) -> StanMenu {
        StanMenu {
            widoczna: m.widoczna,
            pauza: m.pauza,
            koniec_pracy: m.koniec_pracy,
            ukryj_przed_nagrywaniem,
            dzwiek,
            postac: postac.to_string(),
        }
    }
}

/// Pozycja „Zamknij {postac}” w obu menu.
pub fn etykieta_zamkniecia(postac: &str) -> String {
    format!("Zamknij {postac}")
}

/// Pozycja przełączająca dźwięk skupienia w obu menu.
pub const ETYKIETA_DZWIEKU: &str = "Dźwięk 40 Hz";

pub struct Zasobnik {
    pokaz: MenuItem<Wry>,
    pauza: MenuItem<Wry>,
    koniec: MenuItem<Wry>,
    nagrywanie: CheckMenuItem<Wry>,
    dzwiek: CheckMenuItem<Wry>,
    zamknij: MenuItem<Wry>,
    ikona: TrayIcon<Wry>,
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
        let przywroc = MenuItem::with_id(app, PRZYWROC, "Przywróć pozycję", true, None::<&str>)?;
        let nagrywanie = CheckMenuItem::with_id(
            app,
            NAGRYWANIE,
            "Ukryj przed nagrywaniem",
            true,
            stan.ukryj_przed_nagrywaniem,
            None::<&str>,
        )?;
        let dzwiek = CheckMenuItem::with_id(app, DZWIEK, ETYKIETA_DZWIEKU, true, stan.dzwiek, None::<&str>)?;
        let ustawienia = MenuItem::with_id(app, USTAWIENIA, "Ustawienia", true, None::<&str>)?;
        let zamknij = MenuItem::with_id(app, ZAMKNIJ, etykieta_zamkniecia(&stan.postac), true, None::<&str>)?;
        let menu = Menu::with_items(
            app,
            &[
                &pokaz,
                &pauza,
                &koniec,
                &przywroc,
                &PredefinedMenuItem::separator(app)?,
                &nagrywanie,
                &dzwiek,
                &ustawienia,
                &PredefinedMenuItem::separator(app)?,
                &zamknij,
            ],
        )?;
        let mut budowniczy = TrayIconBuilder::with_id("asystentka")
            .tooltip(&stan.postac)
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
        let zasobnik = Zasobnik { pokaz, pauza, koniec, nagrywanie, dzwiek, zamknij, ikona, ostatni: Mutex::new(None) };
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
        if poprzedni.as_ref().map(|p| p.dzwiek) != Some(stan.dzwiek) {
            let _ = self.dzwiek.set_checked(stan.dzwiek);
        }
        // Nowe imię postaci od razu w menu i w podpowiedzi ikony, bez restartu.
        if poprzedni.as_ref().map(|p| &p.postac) != Some(&stan.postac) {
            let _ = self.zamknij.set_text(etykieta_zamkniecia(&stan.postac));
            let _ = self.ikona.set_tooltip(Some(stan.postac.as_str()));
        }
    }

    pub fn nagrywanie_zaznaczone(&self) -> Option<bool> {
        self.nagrywanie.is_checked().ok()
    }

    /// Kliknięcie przełącza ptaszka samo; po zapisie stanu ustawiamy go tak, jak jest naprawdę.
    pub fn ustaw_dzwiek(&self, zaznaczony: bool) {
        let _ = self.dzwiek.set_checked(zaznaczony);
    }
}

/// Menu pod prawym przyciskiem na postaci i panelu: Koniec pracy na dziś ·
/// Zmień plan · Dodaj zadanie · Pauza / Wznów · Ukryj · Przywróć pozycję ·
/// Dźwięk 40 Hz · Ustawienia · Zamknij {postac}. Pozycje, których w danym stanie nie ma sensu używać,
/// są wyszarzone.
pub fn menu_kontekstowe(app: &AppHandle, m: &Mozliwosci, postac: &str, dzwiek: bool) -> tauri::Result<Menu<Wry>> {
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
    let przywroc = MenuItem::with_id(app, CTX_PRZYWROC, "Przywróć pozycję", true, None::<&str>)?;
    let dzwiek = CheckMenuItem::with_id(app, CTX_DZWIEK, ETYKIETA_DZWIEKU, true, dzwiek, None::<&str>)?;
    let ustawienia = MenuItem::with_id(app, CTX_USTAWIENIA, "Ustawienia", true, None::<&str>)?;
    let zamknij = MenuItem::with_id(app, CTX_ZAMKNIJ, etykieta_zamkniecia(postac), true, None::<&str>)?;
    Menu::with_items(
        app,
        &[
            &koniec,
            &plan,
            &dodaj,
            &PredefinedMenuItem::separator(app)?,
            &pauza,
            &ukryj,
            &przywroc,
            &PredefinedMenuItem::separator(app)?,
            &dzwiek,
            &ustawienia,
            &zamknij,
        ],
    )
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn zamkniecie_nosi_imie_postaci() {
        assert_eq!(etykieta_zamkniecia("Sai"), "Zamknij Sai");
        assert_eq!(etykieta_zamkniecia("才ちゃん"), "Zamknij 才ちゃん");
    }

    #[test]
    fn stan_menu_niesie_imie_postaci_i_zmiana_imienia_to_zmiana_stanu() {
        let m = Mozliwosci { koniec_pracy: true, zmien_plan: true, dodaj_zadanie: true, pauza: None, widoczna: true };
        let a = StanMenu::z(&m, true, true, "Sai");
        let b = StanMenu::z(&m, true, true, "Bajtka");
        assert_eq!(a.postac, "Sai");
        assert_ne!(a, b, "odświeżenie menu zauważy nowe imię");
    }

    #[test]
    fn stan_menu_niesie_przelacznik_dzwieku() {
        let m = Mozliwosci { koniec_pracy: true, zmien_plan: true, dodaj_zadanie: true, pauza: None, widoczna: true };
        let z = StanMenu::z(&m, true, true, "Sai");
        let bez = StanMenu::z(&m, true, false, "Sai");
        assert!(z.dzwiek && !bez.dzwiek);
        assert_ne!(z, bez, "odświeżenie menu zauważy włączenie i wyłączenie dźwięku");
        assert_eq!(ETYKIETA_DZWIEKU, "Dźwięk 40 Hz");
    }
}
