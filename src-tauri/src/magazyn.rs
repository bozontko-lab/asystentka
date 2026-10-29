//! Lokalne pliki widgetu w `%APPDATA%\Asystentka`: ustawienia, stan
//! (sesja, prawdziwy stan zadań, kotwica okna, kolejka zapisów) i rejestr
//! sesji każdego dnia, z którego odtwarzany jest blok w dzienniku.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::czas::{self, Chwila};
use crate::dziennik::DzienPracy;
use crate::pliki;
use crate::sesja::{InfoZadania, Sesja};
use crate::zadania::StanZadan;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Strona {
    #[default]
    Prawa,
    Lewa,
}

/// Pozycja okna jako odległość od krawędzi obszaru roboczego monitora
/// głównego, w pikselach logicznych. Dolna krawędź okna zostaje na miejscu,
/// gdy okno zmienia rozmiar.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct KotwicaOkna {
    pub strona: Strona,
    pub odstep_x: f64,
    pub odstep_dol: f64,
}

impl Default for KotwicaOkna {
    fn default() -> Self {
        KotwicaOkna { strona: Strona::Prawa, odstep_x: 16.0, odstep_dol: 16.0 }
    }
}

/// Zapisy, które jeszcze nie trafiły na Dysk albo do vaultu.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DoZapisu {
    pub stan_zadan: bool,
    pub dzienniki: BTreeSet<NaiveDate>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct StanLokalny {
    pub ostatni_zapis: Option<Chwila>,
    pub ostatnie_powitanie: Option<NaiveDate>,
    pub koniec_dnia_pokazany: Option<NaiveDate>,
    pub okno: KotwicaOkna,
    pub sesja: Option<Sesja>,
    pub zadania: StanZadan,
    /// Nazwy i szacunki zadań z planów, żeby pokazać zadania „W toku”, zanim przyjdzie nowy plan.
    pub znane_zadania: BTreeMap<String, InfoZadania>,
    pub do_zapisu: DoZapisu,
}

pub struct Magazyn {
    pub katalog: PathBuf,
}

impl Magazyn {
    pub fn new(katalog: PathBuf) -> Magazyn {
        Magazyn { katalog }
    }

    pub fn ustawienia(&self) -> PathBuf {
        self.katalog.join("ustawienia.json")
    }

    fn stan(&self) -> PathBuf {
        self.katalog.join("stan.json")
    }

    fn dzien(&self, data: NaiveDate) -> PathBuf {
        self.katalog.join("dni").join(format!("{}.json", czas::data_iso(data)))
    }

    /// Brak pliku to pierwszy start. Uszkodzony plik zostaje odłożony obok
    /// jako `stan.json.uszkodzony`, żeby dało się go obejrzeć.
    pub fn wczytaj_stan(&self) -> StanLokalny {
        let sciezka = self.stan();
        match pliki::czytaj_tekst(&sciezka) {
            Ok(tekst) => serde_json::from_str(&tekst).unwrap_or_else(|e| {
                log::error!("{}: uszkodzony stan ({e}), zaczynam od zera", sciezka.display());
                let _ = fs::rename(&sciezka, self.katalog.join("stan.json.uszkodzony"));
                StanLokalny::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StanLokalny::default(),
            Err(e) => {
                log::error!("{}: nie mogę przeczytać stanu: {e}", sciezka.display());
                StanLokalny::default()
            }
        }
    }

    pub fn zapisz_stan(&self, stan: &StanLokalny) -> Result<(), String> {
        fs::create_dir_all(&self.katalog).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(stan).map_err(|e| e.to_string())?;
        pliki::zapisz_atomowo(&self.stan(), json.as_bytes()).map_err(|e| e.to_string())
    }

    pub fn wczytaj_dzien(&self, data: NaiveDate) -> DzienPracy {
        let sciezka = self.dzien(data);
        match pliki::czytaj_tekst(&sciezka) {
            Ok(tekst) => match serde_json::from_str::<DzienPracy>(&tekst) {
                Ok(dzien) if dzien.data == data => dzien,
                Ok(_) | Err(_) => {
                    log::error!("{}: uszkodzony rejestr dnia", sciezka.display());
                    DzienPracy::nowy(data)
                }
            },
            Err(_) => DzienPracy::nowy(data),
        }
    }

    pub fn zapisz_dzien(&self, dzien: &DzienPracy) -> Result<(), String> {
        let sciezka = self.dzien(dzien.data);
        if let Some(folder) = sciezka.parent() {
            fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(dzien).map_err(|e| e.to_string())?;
        pliki::zapisz_atomowo(&sciezka, json.as_bytes()).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::czas::chwila;

    #[test]
    fn stan_przezywa_zapis_i_odczyt() {
        let katalog = tempfile::tempdir().unwrap();
        let magazyn = Magazyn::new(katalog.path().join("Asystentka"));
        assert_eq!(magazyn.wczytaj_stan(), StanLokalny::default());
        let mut stan = StanLokalny { ostatni_zapis: Some(chwila("2026-10-30T10:00:00+01:00")), ..Default::default() };
        stan.do_zapisu.dzienniki.insert(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        magazyn.zapisz_stan(&stan).unwrap();
        assert_eq!(magazyn.wczytaj_stan(), stan);
    }

    #[test]
    fn uszkodzony_stan_jest_odkladany() {
        let katalog = tempfile::tempdir().unwrap();
        let magazyn = Magazyn::new(katalog.path().to_path_buf());
        fs::write(katalog.path().join("stan.json"), "{ zepsute").unwrap();
        assert_eq!(magazyn.wczytaj_stan(), StanLokalny::default());
        assert!(katalog.path().join("stan.json.uszkodzony").exists());
    }

    #[test]
    fn rejestr_dnia() {
        let katalog = tempfile::tempdir().unwrap();
        let magazyn = Magazyn::new(katalog.path().to_path_buf());
        let data = NaiveDate::from_ymd_opt(2026, 10, 30).unwrap();
        assert_eq!(magazyn.wczytaj_dzien(data), DzienPracy::nowy(data));
        let dzien = DzienPracy::nowy(data);
        magazyn.zapisz_dzien(&dzien).unwrap();
        assert_eq!(magazyn.wczytaj_dzien(data), dzien);
    }
}
