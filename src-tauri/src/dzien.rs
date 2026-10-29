//! Stan dnia w `dzien.json` (SPEC.md, B3), żeby restart komputera niczego
//! nie gubił: plan na dziś, trwająca sesja, rejestr zakończonych sesji i
//! zapisy, które jeszcze nie trafiły do plików.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::dziennik::{DzienPracy, WpisSesji};
use crate::plik_zadan::Klucz;
use crate::sesja::Sesja;

/// Czas i ewentualny ptaszek, które czekają na zapis w `Zadania.md`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edycja {
    #[serde(flatten)]
    pub klucz: Klucz,
    pub minuty: u32,
    /// Data ✅, gdy sesja zakończyła się przyciskiem „Zrobione”.
    pub zrobione: Option<NaiveDate>,
}

/// Reakcja na koniec dnia (SPEC.md, B4): przepracowany czas względem celu
/// i to, czy plan jest wykonany.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reakcja {
    Zachwycona,
    Zadowolona,
    Dumna,
    LekkoPochwalajaca,
    Czula,
    Zmartwiona,
}

impl Reakcja {
    pub fn dla(czas_min: u32, cel_min: u32, plan_wykonany: bool) -> Reakcja {
        match (czas_min >= cel_min, czas_min >= 60, plan_wykonany) {
            (true, _, true) => Reakcja::Zachwycona,
            (true, _, false) => Reakcja::Zadowolona,
            (false, true, true) => Reakcja::Dumna,
            (false, true, false) => Reakcja::LekkoPochwalajaca,
            (false, false, true) => Reakcja::Czula,
            (false, false, false) => Reakcja::Zmartwiona,
        }
    }

    /// Nazwa do dziennika.
    pub fn nazwa(self) -> &'static str {
        match self {
            Reakcja::Zachwycona => "zachwycona",
            Reakcja::Zadowolona => "zadowolona",
            Reakcja::Dumna => "dumna",
            Reakcja::LekkoPochwalajaca => "lekko pochwalająca",
            Reakcja::Czula => "czuła",
            Reakcja::Zmartwiona => "zmartwiona",
        }
    }

    /// Klucz dymku w `src/kwestie.json`.
    pub fn klucz_dymku(self) -> &'static str {
        match self {
            Reakcja::Zachwycona => "koniec_zachwycona",
            Reakcja::Zadowolona => "koniec_zadowolona",
            Reakcja::Dumna => "koniec_dumna",
            Reakcja::LekkoPochwalajaca => "koniec_pochwala",
            Reakcja::Czula => "koniec_czula",
            Reakcja::Zmartwiona => "koniec_zmartwiona",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DzienStan {
    pub data: NaiveDate,
    pub plan: Vec<Klucz>,
    pub sesja: Option<Sesja>,
    pub zamkniety: bool,
    /// Pełne powitanie tylko przy pierwszym starcie dnia.
    pub powitano: bool,
    pub zapisano: Option<chrono::DateTime<chrono::FixedOffset>>,
    /// Niedokończone zadania z poprzedniego planu, zaznaczone z góry w Wyborze zadań.
    pub przeniesione: Vec<Klucz>,
    /// Zakończone dziś sesje; z nich odtwarzany jest blok w dzienniku.
    pub sesje: Vec<WpisSesji>,
    pub oczekujace: Vec<Edycja>,
    /// Dziennik tego dnia nie zapisał się i czeka na ponowienie.
    pub dziennik_do_zapisu: bool,
    /// Dzienniki wcześniejszych dni, które nie zapisały się przy zamknięciu.
    pub zalegle: Vec<DzienPracy>,
    /// Po „Wróć do pracy” dzień nie zamyka się już sam o godzinie zamknięcia.
    pub nie_zamykaj_automatycznie: bool,
    pub reakcja: Option<Reakcja>,
}

impl Default for DzienStan {
    fn default() -> Self {
        DzienStan {
            data: NaiveDate::from_ymd_opt(1970, 1, 1).expect("poprawna data"),
            plan: Vec::new(),
            sesja: None,
            zamkniety: false,
            powitano: false,
            zapisano: None,
            przeniesione: Vec::new(),
            sesje: Vec::new(),
            oczekujace: Vec::new(),
            dziennik_do_zapisu: false,
            zalegle: Vec::new(),
            nie_zamykaj_automatycznie: false,
            reakcja: None,
        }
    }
}

impl DzienStan {
    pub fn nowy(data: NaiveDate) -> DzienStan {
        DzienStan { data, ..DzienStan::default() }
    }

    pub fn minuty_sesji(&self) -> u32 {
        self.sesje.iter().map(|s| s.minuty).sum()
    }
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn szesc_reakcji_z_tabeli_b4() {
        let cel = 240;
        assert_eq!(Reakcja::dla(240, cel, true), Reakcja::Zachwycona);
        assert_eq!(Reakcja::dla(300, cel, false), Reakcja::Zadowolona);
        assert_eq!(Reakcja::dla(239, cel, true), Reakcja::Dumna);
        assert_eq!(Reakcja::dla(60, cel, false), Reakcja::LekkoPochwalajaca);
        assert_eq!(Reakcja::dla(59, cel, true), Reakcja::Czula);
        assert_eq!(Reakcja::dla(0, cel, false), Reakcja::Zmartwiona);
        assert_eq!(Reakcja::LekkoPochwalajaca.nazwa(), "lekko pochwalająca");
        assert_eq!(Reakcja::LekkoPochwalajaca.klucz_dymku(), "koniec_pochwala");
    }

    #[test]
    fn stan_dnia_przezywa_zapis_i_odczyt_z_polami_jak_w_b3() {
        let mut d = DzienStan::nowy(NaiveDate::from_ymd_opt(2026, 10, 30).unwrap());
        d.plan = vec![Klucz::new("Film #1 · FIFA", "Miniatura")];
        let json = serde_json::to_string(&d).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["data"], "2026-10-30");
        assert_eq!(v["plan"][0]["projekt"], "Film #1 · FIFA");
        assert_eq!(v["plan"][0]["zadanie"], "Miniatura");
        assert_eq!(v["zamkniety"], false);
        assert_eq!(serde_json::from_str::<DzienStan>(&json).unwrap(), d);
    }
}
