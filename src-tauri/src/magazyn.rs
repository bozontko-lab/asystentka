//! Własne pliki widgetu w `%APPDATA%\Asystentka` (SPEC.md, B3): `dzien.json`
//! ze stanem dnia i `okno.json` z położeniem okna. Ustawienia obsługuje
//! moduł `ustawienia`.

use std::fs;
use std::path::PathBuf;

use chrono::NaiveDate;

use crate::czas::{self, Chwila};
use crate::dzien::DzienStan;
use crate::dziennik::WpisSesji;
use crate::okno::KotwicaOkna;
use crate::pliki;

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

    fn plik_dnia(&self) -> PathBuf {
        self.katalog.join("dzien.json")
    }

    fn plik_okna(&self) -> PathBuf {
        self.katalog.join("okno.json")
    }

    /// Brak pliku to pierwszy start. Uszkodzony plik zostaje odłożony obok
    /// jako `dzien.json.uszkodzony`, żeby dało się go obejrzeć.
    pub fn wczytaj_dzien(&self) -> Option<DzienStan> {
        let sciezka = self.plik_dnia();
        match pliki::czytaj_tekst(&sciezka) {
            Ok(tekst) => match serde_json::from_str(&tekst) {
                Ok(d) => Some(d),
                Err(e) => {
                    log::error!("{}: uszkodzony stan dnia ({e}), zaczynam od zera", sciezka.display());
                    let _ = fs::rename(&sciezka, self.katalog.join("dzien.json.uszkodzony"));
                    None
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                log::error!("{}: nie mogę przeczytać stanu dnia: {e}", sciezka.display());
                None
            }
        }
    }

    pub fn zapisz_dzien(&self, dzien: &DzienStan) -> Result<(), String> {
        fs::create_dir_all(&self.katalog).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(dzien).map_err(|e| e.to_string())?;
        pliki::zapisz_atomowo(&self.plik_dnia(), json.as_bytes()).map_err(|e| e.to_string())
    }

    /// Położenie okna (monitor, róg i odstępy); w wersji 1 leżało w `stan.json`.
    pub fn wczytaj_okno(&self) -> KotwicaOkna {
        if let Ok(tekst) = pliki::czytaj_tekst(&self.plik_okna()) {
            if let Ok(k) = serde_json::from_str(&tekst) {
                return k;
            }
        }
        pliki::czytaj_tekst(&self.katalog.join("stan.json"))
            .ok()
            .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            .and_then(|v| serde_json::from_value(v.get("okno")?.clone()).ok())
            .unwrap_or_default()
    }

    pub fn zapisz_okno(&self, kotwica: &KotwicaOkna) -> Result<(), String> {
        fs::create_dir_all(&self.katalog).map_err(|e| e.to_string())?;
        let json = serde_json::to_string_pretty(kotwica).map_err(|e| e.to_string())?;
        pliki::zapisz_atomowo(&self.plik_okna(), json.as_bytes()).map_err(|e| e.to_string())
    }

    /// Sesje z rejestru wersji 1 (`dni/RRRR-MM-DD.json`), żeby przejście na
    /// wersję 2 w środku dnia nie zgubiło linii z dziennika.
    pub fn sesje_z_wersji_1(&self, data: NaiveDate) -> Vec<WpisSesji> {
        let plik = self.katalog.join("dni").join(format!("{}.json", czas::data_iso(data)));
        let Ok(tekst) = pliki::czytaj_tekst(&plik) else { return Vec::new() };
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&tekst) else { return Vec::new() };
        let czas_z = |p: &serde_json::Value, klucz: &str| -> Option<Chwila> {
            chrono::DateTime::parse_from_rfc3339(p.get(klucz)?.as_str()?).ok()
        };
        v.get("sesje")
            .and_then(|s| s.as_array())
            .map(|sesje| {
                sesje
                    .iter()
                    .filter_map(|s| {
                        Some(WpisSesji {
                            projekt: String::new(),
                            zadanie: s.get("nazwa")?.as_str()?.to_string(),
                            szac_min: s.get("szac_min").and_then(|m| m.as_u64()).map(|m| m as u32),
                            start: czas_z(s, "start")?,
                            koniec: czas_z(s, "koniec")?,
                            sekundy: s.get("sekundy").and_then(|m| m.as_i64()).unwrap_or(0),
                            minuty: s.get("minuty").and_then(|m| m.as_u64()).unwrap_or(0) as u32,
                            zrobione: s.get("status").and_then(|m| m.as_str()) == Some("Zrobione"),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod testy {
    use super::*;
    use crate::okno::{MonitorOkna, Naroznik};

    #[test]
    fn stan_dnia_i_okno_przezywaja_zapis_i_odczyt() {
        let katalog = tempfile::tempdir().unwrap();
        let m = Magazyn::new(katalog.path().join("Asystentka"));
        assert!(m.wczytaj_dzien().is_none());
        assert_eq!(m.wczytaj_okno(), KotwicaOkna::default());
        let dzien = DzienStan::nowy(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        m.zapisz_dzien(&dzien).unwrap();
        assert_eq!(m.wczytaj_dzien(), Some(dzien));
        let kotwica = KotwicaOkna {
            monitor: Some(MonitorOkna { nazwa: "\\\\.\\DISPLAY2".into(), x: 1920, y: 0, szer: 2560, wys: 1440 }),
            kotwica: Naroznik::LewyGorny,
            odstep_x: 40.0,
            odstep_y: 12.5,
        };
        m.zapisz_okno(&kotwica).unwrap();
        assert_eq!(m.wczytaj_okno(), kotwica);
    }

    #[test]
    fn uszkodzony_stan_dnia_jest_odkladany() {
        let katalog = tempfile::tempdir().unwrap();
        let m = Magazyn::new(katalog.path().to_path_buf());
        fs::write(katalog.path().join("dzien.json"), "{ zepsute").unwrap();
        assert!(m.wczytaj_dzien().is_none());
        assert!(katalog.path().join("dzien.json.uszkodzony").exists());
    }

    #[test]
    fn okno_i_sesje_z_wersji_1_sa_przejmowane() {
        let katalog = tempfile::tempdir().unwrap();
        let m = Magazyn::new(katalog.path().to_path_buf());
        fs::write(
            katalog.path().join("stan.json"),
            r#"{"okno": {"strona": "lewa", "odstep_x": 30.0, "odstep_dol": 20.0}, "sesja": null}"#,
        )
        .unwrap();
        let okno = m.wczytaj_okno();
        assert_eq!((okno.kotwica, okno.odstep_x, okno.odstep_y), (Naroznik::LewyDolny, 30.0, 20.0));
        assert!(okno.monitor.is_none(), "w wersji 1 widget stał na monitorze głównym");
        fs::create_dir_all(katalog.path().join("dni")).unwrap();
        fs::write(
            katalog.path().join("dni").join("2026-10-30.json"),
            r##"{"data": "2026-10-30", "sesje": [{"id": "12", "nazwa": "Scenariusz: intro", "szac_min": 60,
              "start": "2026-10-30T09:12:00+01:00", "koniec": "2026-10-30T10:05:00+01:00",
              "sekundy": 2820, "minuty": 47, "status": "Zrobione"}]}"##,
        )
        .unwrap();
        let sesje = m.sesje_z_wersji_1(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        assert_eq!(sesje.len(), 1);
        assert_eq!((sesje[0].zadanie.as_str(), sesje[0].minuty, sesje[0].zrobione), ("Scenariusz: intro", 47, true));
        assert!(m.sesje_z_wersji_1(NaiveDate::from_ymd_opt(2026, 10, 31).unwrap()).is_empty());
    }
}
