//! Skrót globalny pokazujący i chowający widget (SPEC.md, B6).
//!
//! Ctrl+Alt+Spacja otwiera szybkie okno aplikacji Claude, więc domyślny skrót to
//! Ctrl+Alt+Shift+Spacja. Ctrl+Alt z literą to na polskiej klawiaturze AltGr, więc
//! taki skrót blokowałby pisanie ą, ć, ę, ł, ń, ó, ś, ź i ż: widget go odrzuca.

use std::str::FromStr;

use tauri_plugin_global_shortcut::{Code, Modifiers, Shortcut};

pub const DOMYSLNY: &str = "Ctrl+Alt+Shift+Space";
/// Domyślny skrót z wersji 1 i 2. Zajmuje go szybkie okno aplikacji Claude,
/// więc widget zamienia go na nowy przy pierwszym starcie wersji 2.1.
pub const STARY_DOMYSLNY: &str = "Ctrl+Alt+Space";

/// Skrót, który przeszedł sprawdzenie: zapis w jednej, kanonicznej postaci
/// (np. `Ctrl+Alt+Shift+Space`) i skrót gotowy do rejestracji.
#[derive(Debug, Clone, PartialEq)]
pub struct Skrot {
    pub tekst: String,
    pub skrot: Shortcut,
}

/// Czy to stary domyślny skrót, w dowolnym zapisie (`ctrl + alt + space`, `Control+Alt+Space`).
pub fn to_stary_domyslny(tekst: &str) -> bool {
    match (Shortcut::from_str(tekst.trim()), Shortcut::from_str(STARY_DOMYSLNY)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Sprawdza skrót tak, jak sprawdza go pole w ustawieniach: musi być zrozumiały,
/// mieć modyfikator i nie może zabierać klawiszy do pisania. Rejestracji w
/// systemie (czy skrót jest wolny) nie sprawdza: to robi warstwa Tauri.
pub fn analizuj(tekst: &str) -> Result<Skrot, String> {
    let tekst = tekst.trim();
    if tekst.is_empty() {
        return Err("Wybierz skrót klawiszowy, np. Ctrl+Alt+Shift+Spacja.".to_string());
    }
    let skrot = Shortcut::from_str(tekst)
        .map_err(|_| format!("Nie rozumiem skrótu „{tekst}”. Przykład: Ctrl+Alt+Shift+Space."))?;
    let klawisz = skrot.key.to_string();
    if skrot.mods.is_empty() {
        return Err("Skrót musi zawierać Ctrl, Alt, Shift albo Win: sam klawisz zablokowałby pisanie.".to_string());
    }
    if skrot.mods.contains(Modifiers::CONTROL) && skrot.mods.contains(Modifiers::ALT) && jest_litera(&klawisz) {
        return Err(
            "Ctrl+Alt z literą to na polskiej klawiaturze AltGr: taki skrót zablokowałby pisanie ą, ć, ę, ł, ń, ó, ś, ź i ż."
                .to_string(),
        );
    }
    if skrot.mods == Modifiers::SHIFT && do_pisania(&klawisz) {
        return Err(
            "Sam Shift z literą, cyfrą albo spacją zablokowałby pisanie tych znaków w każdym programie.".to_string()
        );
    }
    Ok(Skrot { tekst: kanoniczny(&skrot), skrot })
}

fn jest_litera(klawisz: &str) -> bool {
    klawisz.strip_prefix("Key").is_some_and(|l| l.len() == 1)
}

/// Klawisze, które coś wpisują: litery, cyfry, spacja i znaki interpunkcyjne.
fn do_pisania(klawisz: &str) -> bool {
    jest_litera(klawisz)
        || klawisz.strip_prefix("Digit").is_some_and(|c| c.len() == 1)
        || matches!(
            klawisz,
            "Space"
                | "Minus"
                | "Equal"
                | "BracketLeft"
                | "BracketRight"
                | "Backslash"
                | "Semicolon"
                | "Quote"
                | "Comma"
                | "Period"
                | "Slash"
                | "Backquote"
        )
}

fn nazwa_klawisza(klawisz: Code) -> String {
    let nazwa = klawisz.to_string();
    for przedrostek in ["Key", "Digit"] {
        if let Some(reszta) = nazwa.strip_prefix(przedrostek) {
            if reszta.len() == 1 {
                return reszta.to_string();
            }
        }
    }
    nazwa
}

fn kanoniczny(skrot: &Shortcut) -> String {
    let mut czesci: Vec<String> = Vec::new();
    for (modyfikator, nazwa) in [
        (Modifiers::CONTROL, "Ctrl"),
        (Modifiers::ALT, "Alt"),
        (Modifiers::SHIFT, "Shift"),
        (Modifiers::SUPER, "Super"),
    ] {
        if skrot.mods.contains(modyfikator) {
            czesci.push(nazwa.to_string());
        }
    }
    czesci.push(nazwa_klawisza(skrot.key));
    czesci.join("+")
}

/// Skrót do pokazania człowiekowi: `Ctrl+Alt+Shift+Spacja`.
pub fn dla_czlowieka(tekst: &str) -> String {
    tekst
        .split('+')
        .map(|c| match c.trim() {
            "Space" => "Spacja",
            "Super" => "Win",
            "Escape" => "Esc",
            "ArrowUp" => "↑",
            "ArrowDown" => "↓",
            "ArrowLeft" => "←",
            "ArrowRight" => "→",
            inne => inne,
        })
        .collect::<Vec<_>>()
        .join("+")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn domyslny_skrot_przechodzi_sprawdzenie_i_ma_kanoniczny_zapis() {
        let s = analizuj(DOMYSLNY).unwrap();
        assert_eq!(s.tekst, "Ctrl+Alt+Shift+Space");
        assert_eq!(s.skrot, Shortcut::from_str("ctrl + shift + alt + space").unwrap());
        assert_eq!(analizuj("  control+alt+shift+SPACE ").unwrap().tekst, DOMYSLNY);
    }

    #[test]
    fn kanoniczny_zapis_zawsze_da_sie_wczytac_z_powrotem() {
        for tekst in
            ["Ctrl+Shift+F9", "Alt+Up", "Ctrl+Alt+Shift+Digit5", "Super+Backquote", "Ctrl+Alt+Numpad0", "Ctrl+Pause"]
        {
            let s = analizuj(tekst).unwrap_or_else(|e| panic!("{tekst}: {e}"));
            assert_eq!(analizuj(&s.tekst).unwrap(), s, "{tekst} → {}", s.tekst);
        }
        assert_eq!(analizuj("Ctrl+Alt+5").unwrap().tekst, "Ctrl+Alt+5");
        assert_eq!(analizuj("Ctrl+KeyK").unwrap().tekst, "Ctrl+K");
    }

    #[test]
    fn ctrl_alt_z_litera_jest_odrzucony_bo_to_altgr() {
        for tekst in ["Ctrl+Alt+A", "Ctrl+Alt+Shift+L", "alt+ctrl+z", "Ctrl+Alt+KeyO"] {
            let blad = analizuj(tekst).unwrap_err();
            assert!(blad.contains("AltGr") && blad.contains("ą, ć, ę, ł, ń, ó, ś, ź i ż"), "{tekst}: {blad}");
        }
    }

    #[test]
    fn ctrl_alt_ze_spacja_cyfra_albo_klawiszem_funkcyjnym_jest_dozwolony() {
        for tekst in ["Ctrl+Alt+Space", "Ctrl+Alt+Shift+Space", "Ctrl+Alt+7", "Ctrl+Alt+F9", "Ctrl+Alt+Up"] {
            assert!(analizuj(tekst).is_ok(), "{tekst}");
        }
    }

    #[test]
    fn skrot_bez_modyfikatora_albo_z_samym_shiftem_zablokowalby_pisanie() {
        for tekst in ["F9", "Space", "A", "Escape"] {
            assert!(analizuj(tekst).unwrap_err().contains("Ctrl, Alt, Shift albo Win"), "{tekst}");
        }
        for tekst in ["Shift+A", "Shift+Space", "Shift+4", "Shift+Comma"] {
            assert!(analizuj(tekst).unwrap_err().contains("Sam Shift"), "{tekst}");
        }
        assert!(analizuj("Shift+F9").is_ok(), "Shift z klawiszem funkcyjnym nic nie zabiera");
    }

    #[test]
    fn nonsens_i_pusty_skrot_dostaja_komunikat() {
        assert!(analizuj("").unwrap_err().contains("Wybierz skrót"));
        assert!(analizuj("Ctrl+Alt+").unwrap_err().contains("Nie rozumiem"));
        assert!(analizuj("Ctrl+Pomidor").unwrap_err().contains("Nie rozumiem"));
        assert!(analizuj("Ctrl+A+B").unwrap_err().contains("Nie rozumiem"));
    }

    #[test]
    fn stary_domyslny_jest_rozpoznawany_w_kazdym_zapisie() {
        for tekst in ["Ctrl+Alt+Space", "ctrl+alt+space", "Control + Alt + Space", "alt+ctrl+Space"] {
            assert!(to_stary_domyslny(tekst), "{tekst}");
        }
        for tekst in ["Ctrl+Alt+Shift+Space", "Ctrl+Space", "", "nonsens"] {
            assert!(!to_stary_domyslny(tekst), "{tekst}");
        }
    }

    #[test]
    fn skrot_dla_czlowieka_ma_polska_spacje() {
        assert_eq!(dla_czlowieka("Ctrl+Alt+Shift+Space"), "Ctrl+Alt+Shift+Spacja");
        assert_eq!(dla_czlowieka("Super+Up"), "Win+Up");
        assert_eq!(dla_czlowieka("Super+ArrowUp"), "Win+↑");
    }
}
