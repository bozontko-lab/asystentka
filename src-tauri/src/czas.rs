//! Czas i formatowanie po polsku.
//!
//! Cała logika pracuje na `Chwila` (czas z przesunięciem strefy), żeby testy
//! nie zależały od strefy czasowej maszyny. Bieżący czas daje tylko `teraz()`.

use chrono::{DateTime, FixedOffset, Local, NaiveDate, NaiveTime, SecondsFormat};

pub type Chwila = DateTime<FixedOffset>;

pub fn teraz() -> Chwila {
    Local::now().fixed_offset()
}

/// Dzień kalendarzowy chwili w jej własnej strefie, czyli lokalnie.
pub fn dzien(chwila: Chwila) -> NaiveDate {
    chwila.date_naive()
}

/// `2026-10-30T12:10:44+01:00`, jak w plikach z SPEC.md.
pub fn rfc3339(chwila: Chwila) -> String {
    chwila.to_rfc3339_opts(SecondsFormat::Secs, false)
}

/// Milisekundy od epoki, dla frontendu (porównania z `Date.now()`).
pub fn ms(chwila: Chwila) -> i64 {
    chwila.timestamp_millis()
}

/// `09:12`
pub fn godzina_minuta(chwila: Chwila) -> String {
    chwila.format("%H:%M").to_string()
}

/// `30.10.2026`
pub fn data_pl(dzien: NaiveDate) -> String {
    dzien.format("%d.%m.%Y").to_string()
}

/// `RRRR-MM-DD`
pub fn data_iso(dzien: NaiveDate) -> String {
    dzien.format("%Y-%m-%d").to_string()
}

/// Czas trwania w minutach jak w dzienniku: `47 min`, `2 h 12 min`, `1 h`.
pub fn minuty_tekst(minuty: u32) -> String {
    let (h, m) = (minuty / 60, minuty % 60);
    match (h, m) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Liczba dni z rzeczownikiem: `1 dzień`, `6 dni`. „Dzień” ma w liczbie
/// mnogiej zawsze formę „dni”, więc wystarczy jeden wyjątek.
pub fn dni_tekst(dni: i64) -> String {
    if dni == 1 {
        "1 dzień".to_string()
    } else {
        format!("{dni} dni")
    }
}

/// Zaokrąglenie sekund pracy do pełnych minut (od 30 s w górę).
pub fn sekundy_na_minuty(sekundy: i64) -> u32 {
    if sekundy <= 0 {
        0
    } else {
        ((sekundy + 30) / 60) as u32
    }
}

/// Godzina z ustawień w formacie `HH:MM`.
pub fn parsuj_godzine(tekst: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(tekst.trim(), "%H:%M").ok()
}

/// Godzina do dymku, bez zera wiodącego: `07:40` → `7:40`.
pub fn godzina_do_dymku(tekst: &str) -> String {
    match parsuj_godzine(tekst) {
        Some(g) => g.format("%-H:%M").to_string(),
        None => tekst.to_string(),
    }
}

#[cfg(test)]
pub fn chwila(tekst: &str) -> Chwila {
    DateTime::parse_from_rfc3339(tekst).expect("poprawna data w teście")
}

#[cfg(test)]
mod testy {
    use super::*;

    #[test]
    fn minuty_jak_w_dzienniku() {
        assert_eq!(minuty_tekst(47), "47 min");
        assert_eq!(minuty_tekst(132), "2 h 12 min");
        assert_eq!(minuty_tekst(60), "1 h");
        assert_eq!(minuty_tekst(0), "0 min");
    }

    #[test]
    fn dni_z_rzeczownikiem() {
        assert_eq!(dni_tekst(1), "1 dzień");
        assert_eq!(dni_tekst(6), "6 dni");
        assert_eq!(dni_tekst(2), "2 dni");
        assert_eq!(dni_tekst(22), "22 dni");
    }

    #[test]
    fn zaokraglanie_minut() {
        assert_eq!(sekundy_na_minuty(0), 0);
        assert_eq!(sekundy_na_minuty(29), 0);
        assert_eq!(sekundy_na_minuty(30), 1);
        assert_eq!(sekundy_na_minuty(47 * 60 + 29), 47);
        assert_eq!(sekundy_na_minuty(-5), 0);
    }

    #[test]
    fn formaty_dat() {
        let c = chwila("2026-10-30T09:12:05+01:00");
        assert_eq!(rfc3339(c), "2026-10-30T09:12:05+01:00");
        assert_eq!(godzina_minuta(c), "09:12");
        assert_eq!(data_pl(dzien(c)), "30.10.2026");
        assert_eq!(data_iso(dzien(c)), "2026-10-30");
    }

    #[test]
    fn godzina_planu_w_dymku() {
        assert_eq!(godzina_do_dymku("07:40"), "7:40");
        assert_eq!(godzina_do_dymku("22:00"), "22:00");
        assert_eq!(godzina_do_dymku("rano"), "rano");
    }
}
