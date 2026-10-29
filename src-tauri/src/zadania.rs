//! Stan zadań `stan-zadan.json` na Dysku (SPEC.md, B3). Widget jest jedynym
//! autorem tego pliku: prawdziwy stan trzyma lokalnie i co zmianę zapisuje
//! jego kopię na Dysk, a poranne zadanie tylko ją czyta.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::czas::{self, Chwila};

pub const W_TOKU: &str = "W toku";
pub const ZROBIONE: &str = "Zrobione";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WpisZadania {
    pub status: String,
    #[serde(default)]
    pub minuty: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zakonczono: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StanZadan {
    pub aktualizacja: Option<String>,
    pub zadania: BTreeMap<String, WpisZadania>,
}

/// Status nigdy się nie cofa: „Zrobione” wygrywa z „W toku”.
fn ranga(status: &str) -> u8 {
    match status {
        ZROBIONE => 2,
        W_TOKU => 1,
        _ => 0,
    }
}

/// ID z arkusza to liczby zapisane tekstem, więc sortujemy liczbowo.
fn porownaj_id(a: &str, b: &str) -> Ordering {
    match (a.parse::<u64>(), b.parse::<u64>()) {
        (Ok(x), Ok(y)) => x.cmp(&y),
        (Ok(_), Err(_)) => Ordering::Less,
        (Err(_), Ok(_)) => Ordering::Greater,
        (Err(_), Err(_)) => a.cmp(b),
    }
}

fn napis(tekst: &str) -> String {
    serde_json::to_string(tekst).expect("napis zawsze da się zserializować")
}

impl StanZadan {
    pub fn wpis(&self, id: &str) -> Option<&WpisZadania> {
        self.zadania.get(id)
    }

    pub fn zrobione(&self, id: &str) -> bool {
        self.wpis(id).is_some_and(|w| w.status == ZROBIONE)
    }

    /// Dolicza minuty po sesji i podnosi status, jeśli trzeba.
    pub fn dolicz(&mut self, id: &str, minuty: u32, status: &str, kiedy: Chwila) {
        let wpis = self.zadania.entry(id.to_string()).or_insert_with(|| WpisZadania {
            status: status.to_string(),
            minuty: 0,
            zakonczono: None,
        });
        wpis.minuty += minuty;
        if ranga(status) > ranga(&wpis.status) {
            wpis.status = status.to_string();
        }
        if status == ZROBIONE && wpis.zakonczono.is_none() {
            wpis.zakonczono = Some(czas::rfc3339(kiedy));
        }
        self.aktualizacja = Some(czas::rfc3339(kiedy));
    }

    /// Przejmuje z pliku na Dysku zadania, których nie ma lokalnie, np. po
    /// reinstalacji widgetu. Lokalne wpisy zawsze wygrywają.
    pub fn uzupelnij_z(&mut self, z_pliku: &StanZadan) -> bool {
        let mut zmiana = false;
        for (id, wpis) in &z_pliku.zadania {
            if !self.zadania.contains_key(id) {
                self.zadania.insert(id.clone(), wpis.clone());
                zmiana = true;
            }
        }
        zmiana
    }

    /// Treść pliku w układzie z SPEC.md: jedno zadanie w linii.
    pub fn do_json(&self) -> String {
        let mut ids: Vec<&String> = self.zadania.keys().collect();
        ids.sort_by(|a, b| porownaj_id(a, b));
        let linie: Vec<String> = ids
            .iter()
            .map(|id| {
                let w = &self.zadania[*id];
                let mut linia =
                    format!("    {}: {{\"status\": {}, \"minuty\": {}", napis(id), napis(&w.status), w.minuty);
                if let Some(z) = &w.zakonczono {
                    linia.push_str(&format!(", \"zakonczono\": {}", napis(z)));
                }
                linia.push('}');
                linia
            })
            .collect();
        let aktualizacja = match &self.aktualizacja {
            Some(a) => napis(a),
            None => "null".to_string(),
        };
        let zadania = if linie.is_empty() { "{}".to_string() } else { format!("{{\n{}\n  }}", linie.join(",\n")) };
        format!("{{\n  \"aktualizacja\": {aktualizacja},\n  \"zadania\": {zadania}\n}}\n")
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::czas::chwila;

    const PRZYKLAD: &str = include_str!("../../przyklady/stan-zadan.json");

    #[test]
    fn format_jak_w_specyfikacji() {
        let stan: StanZadan = serde_json::from_str(PRZYKLAD).unwrap();
        assert_eq!(stan.do_json(), PRZYKLAD);
    }

    #[test]
    fn dolicza_minuty_i_nie_cofa_statusu() {
        let mut stan: StanZadan = serde_json::from_str(PRZYKLAD).unwrap();
        let t = chwila("2026-10-31T09:30:00+01:00");
        stan.dolicz("15", 30, W_TOKU, t);
        assert_eq!(stan.wpis("15").unwrap().minuty, 75);
        stan.dolicz("15", 10, ZROBIONE, t);
        let wpis = stan.wpis("15").unwrap();
        assert_eq!((wpis.status.as_str(), wpis.minuty), (ZROBIONE, 85));
        assert_eq!(wpis.zakonczono.as_deref(), Some("2026-10-31T09:30:00+01:00"));
        stan.dolicz("12", 5, W_TOKU, t);
        let wpis = stan.wpis("12").unwrap();
        assert_eq!((wpis.status.as_str(), wpis.minuty), (ZROBIONE, 52));
        assert_eq!(wpis.zakonczono.as_deref(), Some("2026-10-30T10:05:00+01:00"));
        assert_eq!(stan.aktualizacja.as_deref(), Some("2026-10-31T09:30:00+01:00"));
    }

    #[test]
    fn nowe_zadanie_i_sortowanie_liczbowe() {
        let mut stan = StanZadan::default();
        let t = chwila("2026-10-30T10:00:00+01:00");
        stan.dolicz("100", 5, W_TOKU, t);
        stan.dolicz("9", 40, ZROBIONE, t);
        let json = stan.do_json();
        assert!(json.find("\"9\"").unwrap() < json.find("\"100\"").unwrap());
        let wczytany: StanZadan = serde_json::from_str(&json).unwrap();
        assert_eq!(wczytany, stan);
    }

    #[test]
    fn pusty_stan_to_poprawny_json() {
        let json = StanZadan::default().do_json();
        let _: serde_json::Value = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn uzupelnia_tylko_brakujace() {
        let z_pliku: StanZadan = serde_json::from_str(PRZYKLAD).unwrap();
        let mut lokalny = StanZadan::default();
        lokalny.dolicz("15", 50, W_TOKU, chwila("2026-10-31T09:00:00+01:00"));
        assert!(lokalny.uzupelnij_z(&z_pliku));
        assert_eq!(lokalny.wpis("15").unwrap().minuty, 50);
        assert!(lokalny.zrobione("12"));
        assert!(!lokalny.uzupelnij_z(&z_pliku));
    }
}
