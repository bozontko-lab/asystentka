//! Plan dnia `plan-RRRR-MM-DD.json`, który co rano zapisuje poranne zadanie (SPEC.md, B3).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::SystemTime;

use chrono::NaiveDate;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize};

use crate::czas;
use crate::pliki;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Plan {
    pub data: String,
    pub utworzono: Option<String>,
    pub naglowek: String,
    pub tempo: String,
    pub dni_do_publikacji: Option<i64>,
    pub zadania: Vec<ZadaniePlanu>,
    pub zapasowe: Vec<ZadaniePlanu>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ZadaniePlanu {
    #[serde(deserialize_with = "id_tekstem")]
    pub id: String,
    pub nazwa: String,
    pub etap: Option<String>,
    pub szac_min: Option<u32>,
    pub czesc: Option<String>,
}

impl Plan {
    /// `tempo: "zagrozone"` barwi nagłówek na brzoskwiniowo.
    pub fn zagrozone(&self) -> bool {
        let tempo = self.tempo.trim().to_lowercase();
        tempo == "zagrozone" || tempo == "zagrożone"
    }

    pub fn wszystkie_zadania(&self) -> impl Iterator<Item = &ZadaniePlanu> {
        self.zadania.iter().chain(self.zapasowe.iter())
    }

    pub fn zadanie(&self, id: &str) -> Option<&ZadaniePlanu> {
        self.wszystkie_zadania().find(|z| z.id == id)
    }
}

/// ID wiersza z arkusza ma być tekstem, ale liczba też przejdzie.
fn id_tekstem<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let wartosc = serde_json::Value::deserialize(d)?;
    Ok(match wartosc {
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Null => String::new(),
        inna => inna.to_string(),
    })
}

pub struct ZnalezionyPlan {
    pub plan: Plan,
    pub sciezka: PathBuf,
    pub zmieniony: SystemTime,
}

fn wzorzec_nazwy() -> &'static Regex {
    static WZORZEC: OnceLock<Regex> = OnceLock::new();
    // Dysk może dopisać „ (1)” do nazwy duplikatu.
    WZORZEC
        .get_or_init(|| Regex::new(r"(?i)^plan-(\d{4}-\d{2}-\d{2})\s*(?:\(\d+\))?\.json$").expect("poprawny wzorzec"))
}

pub fn czytaj(sciezka: &Path) -> Result<Plan, String> {
    let tekst = pliki::czytaj_tekst(sciezka).map_err(|e| e.to_string())?;
    serde_json::from_str(&tekst).map_err(|e| e.to_string())
}

/// Najnowszy (według czasu modyfikacji) plik planu na dany dzień, którego
/// nazwa pasuje do wzorca, a pole `data` zgadza się z dniem. Błąd oznacza
/// niedostępny folder, np. gdy aplikacja Dysku jeszcze nie wystartowała.
pub fn znajdz(folder: &Path, dzien: NaiveDate) -> io::Result<Option<ZnalezionyPlan>> {
    let dzis = czas::data_iso(dzien);
    let mut kandydaci = Vec::new();
    for wpis in fs::read_dir(folder)? {
        let Ok(wpis) = wpis else { continue };
        let nazwa = wpis.file_name().to_string_lossy().into_owned();
        let Some(dopasowanie) = wzorzec_nazwy().captures(&nazwa) else { continue };
        if dopasowanie[1] != dzis {
            continue;
        }
        let Ok(meta) = wpis.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let zmieniony = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        kandydaci.push((zmieniony, wpis.path()));
    }
    kandydaci.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
    for (zmieniony, sciezka) in kandydaci {
        match czytaj(&sciezka) {
            Ok(plan) if plan.data.trim() == dzis => return Ok(Some(ZnalezionyPlan { plan, sciezka, zmieniony })),
            Ok(plan) => log::warn!("{}: pole data to {:?}, a nie {dzis}; pomijam", sciezka.display(), plan.data),
            // Plik może być w trakcie synchronizacji; spróbujemy przy kolejnym odczycie.
            Err(e) => log::warn!("{}: nie mogę odczytać planu: {e}", sciezka.display()),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod testy {
    use super::*;
    use std::time::Duration;

    const PRZYKLAD: &str = include_str!("../../przyklady/plan-2026-10-30.json");

    fn dzien() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 30).unwrap()
    }

    fn zapisz(folder: &Path, nazwa: &str, tresc: &str, wiek_s: u64) {
        let sciezka = folder.join(nazwa);
        fs::write(&sciezka, tresc).unwrap();
        let czas = SystemTime::now() - Duration::from_secs(wiek_s);
        fs::File::options().write(true).open(&sciezka).unwrap().set_modified(czas).unwrap();
    }

    #[test]
    fn przyklad_ze_specyfikacji() {
        let plan: Plan = serde_json::from_str(PRZYKLAD).unwrap();
        assert_eq!(plan.data, "2026-10-30");
        assert!(!plan.zagrozone());
        assert_eq!(plan.dni_do_publikacji, Some(6));
        assert_eq!(plan.zadania.len(), 2);
        assert_eq!(plan.zadania[1].czesc.as_deref(), Some("plansze 1–5"));
        assert_eq!(plan.zapasowe[0].id, "18");
        assert_eq!(plan.zadanie("15").unwrap().szac_min, Some(120));
    }

    #[test]
    fn nieznane_pola_i_liczbowe_id() {
        let plan: Plan = serde_json::from_str(
            r#"{"data":"2026-10-30","tempo":"zagrozone","nowe_pole":1,"zadania":[{"id":7,"nazwa":"X","szac_min":null,"cos":true}]}"#,
        )
        .unwrap();
        assert!(plan.zagrozone());
        assert_eq!(plan.zadania[0].id, "7");
        assert_eq!(plan.zadania[0].szac_min, None);
        assert!(plan.zapasowe.is_empty());
    }

    #[test]
    fn wybiera_najnowszy_plik_z_dzisiejsza_data() {
        let katalog = tempfile::tempdir().unwrap();
        let f = katalog.path();
        let stary = PRZYKLAD.replace("Film #1", "Stary");
        let nowy = PRZYKLAD.replace("Film #1", "Nowy");
        zapisz(f, "plan-2026-10-30.json", &stary, 600);
        zapisz(f, "plan-2026-10-30 (1).json", &nowy, 10);
        zapisz(f, "plan-2026-10-29.json", &PRZYKLAD.replace("2026-10-30", "2026-10-29"), 1);
        zapisz(f, "notatki.json", "{}", 0);
        let znaleziony = znajdz(f, dzien()).unwrap().unwrap();
        assert!(znaleziony.plan.naglowek.starts_with("Nowy"));
        assert!(znaleziony.sciezka.ends_with("plan-2026-10-30 (1).json"));
    }

    #[test]
    fn sprawdza_pole_data_i_pomija_uszkodzone() {
        let katalog = tempfile::tempdir().unwrap();
        let f = katalog.path();
        zapisz(f, "plan-2026-10-30.json", PRZYKLAD, 600);
        zapisz(
            f,
            "plan-2026-10-30 (1).json",
            &PRZYKLAD.replace("\"data\": \"2026-10-30\"", "\"data\": \"2026-10-29\""),
            20,
        );
        zapisz(f, "plan-2026-10-30 (2).json", "{ w trakcie synchronizacji", 5);
        let znaleziony = znajdz(f, dzien()).unwrap().unwrap();
        assert!(znaleziony.sciezka.ends_with("plan-2026-10-30.json"));
    }

    #[test]
    fn brak_planu_i_brak_folderu() {
        let katalog = tempfile::tempdir().unwrap();
        assert!(znajdz(katalog.path(), dzien()).unwrap().is_none());
        assert!(znajdz(&katalog.path().join("brak"), dzien()).is_err());
    }
}
