//! Plik zadań `Zadania.md` (SPEC.md, B3). Ty dopisujesz w nim zadania i
//! szacunki, a widget dopisuje czas, ptaszka i datę ukończenia.
//!
//! Plik jest też notatką w Obsidianie, więc widget zmienia w nim jedną linię
//! naraz, czyta plik od nowa tuż przed zapisem i nie rusza reszty: końców
//! linii, BOM-u ani niczego, czego nie rozumie.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::OnceLock;

use chrono::NaiveDate;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::czas;
use crate::pliki;

pub const BEZ_PROJEKTU: &str = "Bez projektu";
pub const INBOX: &str = "Inbox";

/// Zadanie rozpoznaje para: projekt i tekst linii bez pól (SPEC.md, B3).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Klucz {
    pub projekt: String,
    pub zadanie: String,
}

impl Klucz {
    pub fn new(projekt: &str, zadanie: &str) -> Klucz {
        Klucz { projekt: projekt.to_string(), zadanie: zadanie.to_string() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Zadanie {
    pub klucz: Klucz,
    pub zrobione: bool,
    pub szac_min: Option<u32>,
    /// Łączny czas ze wszystkich dni, dopisywany przez widget.
    pub czas_min: u32,
    pub termin: Option<NaiveDate>,
    /// Numer linii w pliku (od 0).
    pub linia: usize,
}

impl Zadanie {
    /// „W toku” to otwarte zadanie z czasem większym od zera, bez osobnego statusu.
    pub fn w_toku(&self) -> bool {
        !self.zrobione && self.czas_min > 0
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlikZadan {
    pub zadania: Vec<Zadanie>,
    /// Nagłówki `##` w kolejności z pliku: (numer linii, nazwa projektu).
    pub naglowki: Vec<(usize, String)>,
    /// Liczba linii pliku.
    pub linii: usize,
}

impl PlikZadan {
    pub fn zadanie(&self, klucz: &Klucz) -> Option<&Zadanie> {
        self.zadania
            .iter()
            .find(|z| &z.klucz == klucz && !z.zrobione)
            .or_else(|| self.zadania.iter().find(|z| &z.klucz == klucz))
    }

    pub fn otwarte(&self) -> impl Iterator<Item = &Zadanie> {
        self.zadania.iter().filter(|z| !z.zrobione)
    }

    /// Projekty do wyboru przy dopisywaniu zadania: nagłówki z pliku, a gdy
    /// nie ma Inboxu, także on (powstanie przy pierwszym zadaniu).
    pub fn projekty_do_dopisania(&self) -> Vec<String> {
        let mut wynik: Vec<String> = Vec::new();
        for (_, nazwa) in &self.naglowki {
            if !wynik.contains(nazwa) {
                wynik.push(nazwa.clone());
            }
        }
        if !wynik.iter().any(|p| p == INBOX) {
            wynik.push(INBOX.to_string());
        }
        wynik
    }
}

fn re_zadanie() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^(\s*[-*+]\s+\[)([ xX])(\]\s+)(.*?)\s*$").expect("poprawny wzorzec"))
}

fn re_naglowek() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"^\s{0,3}##[ \t]+(.+?)[ \t]*$").expect("poprawny wzorzec"))
}

fn re_pole() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\[(\w+)::\s*([^\]]*?)\s*\]").expect("poprawny wzorzec"))
}

fn re_data() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new("[✅📅⏳🛫➕❌]\u{fe0f}?\\s*\\d{4}-\\d{2}-\\d{2}").expect("poprawny wzorzec"))
}

fn re_zrobiono() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new("✅\u{fe0f}?\\s*\\d{4}-\\d{2}-\\d{2}").expect("poprawny wzorzec"))
}

/// `✅ RRRR-MM-DD` razem ze spacjami przed nim.
fn re_ptaszek_z_data() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new("\\s*✅\u{fe0f}?\\s*\\d{4}-\\d{2}-\\d{2}").expect("poprawny wzorzec"))
}

fn re_czas() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\[czas::[^\]]*\]").expect("poprawny wzorzec"))
}

fn re_szac() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)\[szac::[^\]]*\]").expect("poprawny wzorzec"))
}

/// Tekst zadania bez pól `[x:: y]` i dat z wtyczki Tasks, z pojedynczymi spacjami.
pub fn tekst_zadania(linia_po_checkboxie: &str) -> String {
    let bez_pol = re_pole().replace_all(linia_po_checkboxie, " ");
    let bez_dat = re_data().replace_all(&bez_pol, " ");
    bez_dat.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn bez_konca_linii(linia: &str) -> &str {
    linia.trim_end_matches(['\n', '\r'])
}

pub fn parsuj(tekst: &str) -> PlikZadan {
    let mut wynik = PlikZadan::default();
    let mut projekt = BEZ_PROJEKTU.to_string();
    let mut w_komentarzu = false;
    let mut w_bloku_kodu = false;
    for (nr, surowa) in tekst.split_inclusive('\n').enumerate() {
        wynik.linii = nr + 1;
        let linia = bez_konca_linii(surowa);
        if w_komentarzu {
            w_komentarzu = !linia.contains("-->");
            continue;
        }
        let poczatek = linia.trim_start();
        if poczatek.starts_with("```") || poczatek.starts_with("~~~") {
            w_bloku_kodu = !w_bloku_kodu;
            continue;
        }
        if w_bloku_kodu {
            continue;
        }
        if poczatek.starts_with("<!--") {
            w_komentarzu = !poczatek.contains("-->");
            continue;
        }
        if let Some(c) = re_naglowek().captures(linia) {
            let nazwa = c[1].trim_end_matches('#').trim().to_string();
            if !nazwa.is_empty() {
                wynik.naglowki.push((nr, nazwa.clone()));
                projekt = nazwa;
            }
            continue;
        }
        let Some(c) = re_zadanie().captures(linia) else { continue };
        let tresc = &c[4];
        let nazwa = tekst_zadania(tresc);
        if nazwa.is_empty() {
            continue;
        }
        let (mut szac_min, mut czas_min, mut termin) = (None, 0, None);
        for pole in re_pole().captures_iter(tresc) {
            let wartosc = pole[2].trim();
            match pole[1].to_lowercase().as_str() {
                "szac" => szac_min = wartosc.parse::<u32>().ok(),
                "czas" => czas_min = wartosc.parse::<u32>().unwrap_or(0),
                "termin" => termin = NaiveDate::parse_from_str(wartosc, "%Y-%m-%d").ok(),
                _ => {}
            }
        }
        wynik.zadania.push(Zadanie {
            klucz: Klucz { projekt: projekt.clone(), zadanie: nazwa },
            zrobione: !c[2].eq_ignore_ascii_case(" "),
            szac_min,
            czas_min,
            termin,
            linia: nr,
        });
    }
    wynik
}

// ---- edycja ---------------------------------------------------------------------------

#[derive(Debug)]
pub enum BladEdycji {
    /// Zadanie zniknęło z pliku albo zmieniłeś jego tekst.
    NieZnaleziono,
    PustaNazwa,
}

impl std::fmt::Display for BladEdycji {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BladEdycji::NieZnaleziono => write!(f, "nie ma takiego zadania w pliku"),
            BladEdycji::PustaNazwa => write!(f, "nazwa zadania jest pusta"),
        }
    }
}

fn koniec_linii(tekst: &str) -> &'static str {
    if tekst.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Zmienia pojedynczą linię zadania: dodaje minuty do `[czas:: …]` (albo
/// zakłada pole) i, gdy zadanie jest zrobione, zmienia `[ ]` na `[x]` oraz
/// dopisuje `✅ RRRR-MM-DD`.
fn zmien_linie(linia: &str, dodaj_min: u32, zrobione: Option<NaiveDate>) -> String {
    let mut s = linia.to_string();
    if dodaj_min > 0 {
        if let Some(m) = re_czas().find(&s) {
            let obecny = m.as_str().split("::").nth(1).map_or("", |c| c.trim_end_matches(']').trim());
            let nowy = obecny.parse::<u32>().unwrap_or(0) + dodaj_min;
            s.replace_range(m.range(), &format!("[czas:: {nowy}]"));
        } else {
            let pole = format!(" [czas:: {dodaj_min}]");
            if let Some(m) = re_szac().find(&s) {
                s.insert_str(m.end(), &pole);
            } else {
                let pierwsze = [re_pole().find(&s).map(|m| m.start()), re_data().find(&s).map(|m| m.start())]
                    .into_iter()
                    .flatten()
                    .min();
                match pierwsze {
                    Some(start) => {
                        let przed = s[..start].trim_end().len();
                        s.insert_str(przed, &pole);
                    }
                    None => {
                        let koniec = s.trim_end().len();
                        s.truncate(koniec);
                        s.push_str(&pole);
                    }
                }
            }
        }
    }
    if let Some(data) = zrobione {
        if let Some(c) = re_zadanie().captures(&s) {
            let (poczatek, koniec) = (c.get(2).map_or(0, |m| m.start()), c.get(2).map_or(0, |m| m.end()));
            s.replace_range(poczatek..koniec, "x");
        }
        if !re_zrobiono().is_match(&s) {
            let koniec = s.trim_end().len();
            s.truncate(koniec);
            s.push_str(&format!(" ✅ {}", czas::data_iso(data)));
        }
    }
    s
}

/// Dopisuje do zadania czas sesji i, przy `zrobione`, ptaszka z datą.
/// Zmienia tylko jedną linię; reszta tekstu zostaje bajt w bajt taka sama.
pub fn dopisz_czas(
    tekst: &str,
    klucz: &Klucz,
    dodaj_min: u32,
    zrobione: Option<NaiveDate>,
) -> Result<String, BladEdycji> {
    let plik = parsuj(tekst);
    let cel = plik.zadanie(klucz).ok_or(BladEdycji::NieZnaleziono)?.linia;
    let mut linie: Vec<String> = tekst.split_inclusive('\n').map(str::to_string).collect();
    let stara = linie[cel].clone();
    let tresc = bez_konca_linii(&stara);
    let koniec = &stara[tresc.len()..];
    linie[cel] = format!("{}{koniec}", zmien_linie(tresc, dodaj_min, zrobione));
    Ok(linie.concat())
}

/// Cofa „Zrobione”: zmienia `[x]` z powrotem na `[ ]` i usuwa `✅ RRRR-MM-DD`.
/// Czas zostaje. Zadanie, które jest już otwarte (np. odznaczone w Obsidianie),
/// zostaje takie, jakie jest.
pub fn odznacz_zrobione(tekst: &str, klucz: &Klucz) -> Result<String, BladEdycji> {
    let plik = parsuj(tekst);
    let cel = plik
        .zadania
        .iter()
        .find(|z| &z.klucz == klucz && z.zrobione)
        .or_else(|| plik.zadania.iter().find(|z| &z.klucz == klucz))
        .ok_or(BladEdycji::NieZnaleziono)?
        .linia;
    let mut linie: Vec<String> = tekst.split_inclusive('\n').map(str::to_string).collect();
    let stara = linie[cel].clone();
    let tresc = bez_konca_linii(&stara);
    let koniec = &stara[tresc.len()..];
    let mut s = tresc.to_string();
    if let Some(c) = re_zadanie().captures(&s) {
        let (poczatek, koniec_znaku) = (c.get(2).map_or(0, |m| m.start()), c.get(2).map_or(0, |m| m.end()));
        s.replace_range(poczatek..koniec_znaku, " ");
    }
    let bez_daty = re_ptaszek_z_data().replace_all(&s, "").into_owned();
    linie[cel] = format!("{bez_daty}{koniec}");
    Ok(linie.concat())
}

/// Dopisuje linię `- [ ] nazwa [szac:: N]` pod nagłówkiem projektu (po jego
/// ostatnim zadaniu). Brakujący projekt, np. Inbox, powstaje na końcu pliku.
pub fn dopisz_zadanie(
    tekst: &str,
    projekt: &str,
    nazwa: &str,
    szac_min: Option<u32>,
) -> Result<(String, Klucz), BladEdycji> {
    let nazwa = nazwa.split_whitespace().collect::<Vec<_>>().join(" ");
    if nazwa.is_empty() {
        return Err(BladEdycji::PustaNazwa);
    }
    let eol = koniec_linii(tekst);
    let nowa = match szac_min.filter(|m| *m > 0) {
        Some(m) => format!("- [ ] {nazwa} [szac:: {m}]"),
        None => format!("- [ ] {nazwa}"),
    };
    let klucz = Klucz { projekt: projekt.to_string(), zadanie: tekst_zadania(&nazwa) };
    let plik = parsuj(tekst);
    let mut linie: Vec<String> = tekst.split_inclusive('\n').map(str::to_string).collect();

    let naglowek = plik.naglowki.iter().position(|(_, n)| n == projekt);
    let (poczatek, koniec) = match (projekt == BEZ_PROJEKTU, naglowek) {
        (true, _) => (0, plik.naglowki.first().map_or(linie.len(), |(nr, _)| *nr)),
        (false, Some(i)) => (plik.naglowki[i].0 + 1, plik.naglowki.get(i + 1).map_or(linie.len(), |(nr, _)| *nr)),
        (false, None) => {
            // Nowy projekt na końcu pliku, po pustej linii.
            if let Some(ostatnia) = linie.last_mut() {
                if !ostatnia.ends_with('\n') {
                    ostatnia.push_str(eol);
                }
            }
            if linie.last().is_some_and(|l| !bez_konca_linii(l).trim().is_empty()) {
                linie.push(eol.to_string());
            }
            linie.push(format!("## {projekt}{eol}"));
            linie.push(format!("{nowa}{eol}"));
            return Ok((linie.concat(), klucz));
        }
    };
    let ostatnie_zadanie =
        plik.zadania.iter().filter(|z| z.linia >= poczatek && z.linia < koniec).map(|z| z.linia).max();
    let po = match ostatnie_zadanie {
        Some(nr) => nr + 1,
        None => {
            // Brak zadań: po ostatniej niepustej linii sekcji, a w pustej sekcji pod nagłówkiem.
            let ostatnia_niepusta = (poczatek..koniec).rev().find(|nr| !bez_konca_linii(&linie[*nr]).trim().is_empty());
            match (ostatnia_niepusta, projekt == BEZ_PROJEKTU) {
                (Some(nr), _) => nr + 1,
                (None, true) => poczatek,
                (None, false) => poczatek,
            }
        }
    };
    if po > 0 && po <= linie.len() && !linie[po - 1].ends_with('\n') {
        linie[po - 1].push_str(eol);
    }
    linie.insert(po, format!("{nowa}{eol}"));
    Ok((linie.concat(), klucz))
}

// ---- odczyt i zapis pliku ------------------------------------------------------------

#[derive(Debug)]
pub struct Odczyt {
    pub tekst: String,
    pub bom: bool,
}

/// Czyta plik jako UTF-8 i zapamiętuje BOM. Plik w innym kodowaniu to błąd,
/// a nie powód, żeby go „naprawiać”.
pub fn wczytaj(sciezka: &Path) -> io::Result<Odczyt> {
    let bajty = fs::read(sciezka)?;
    let (bom, tresc) = match bajty.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        Some(reszta) => (true, reszta.to_vec()),
        None => (false, bajty),
    };
    let tekst =
        String::from_utf8(tresc).map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "plik nie jest w UTF-8"))?;
    Ok(Odczyt { tekst, bom })
}

pub fn wczytaj_i_parsuj(sciezka: &Path) -> io::Result<PlikZadan> {
    Ok(parsuj(&wczytaj(sciezka)?.tekst))
}

#[derive(Debug)]
pub enum BladZmiany {
    Io(io::Error),
    Edycja(BladEdycji),
}

impl std::fmt::Display for BladZmiany {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BladZmiany::Io(e) => write!(f, "{e}"),
            BladZmiany::Edycja(e) => write!(f, "{e}"),
        }
    }
}

/// Zmienia plik: czyta go od nowa, wywołuje `zmiana` i zapisuje wynik przez
/// plik tymczasowy z zamianą nazwy, ale tylko wtedy, gdy treść się zmieniła.
pub fn zmien<T>(sciezka: &Path, zmiana: impl FnOnce(&str) -> Result<(String, T), BladEdycji>) -> Result<T, BladZmiany> {
    let odczyt = wczytaj(sciezka).map_err(BladZmiany::Io)?;
    let (nowy, wynik) = zmiana(&odczyt.tekst).map_err(BladZmiany::Edycja)?;
    if nowy != odczyt.tekst {
        let mut bajty = Vec::with_capacity(nowy.len() + 3);
        if odczyt.bom {
            bajty.extend_from_slice(&[0xEF, 0xBB, 0xBF]);
        }
        bajty.extend_from_slice(nowy.as_bytes());
        pliki::zapisz_atomowo(sciezka, &bajty).map_err(BladZmiany::Io)?;
    }
    Ok(wynik)
}

#[cfg(test)]
mod testy {
    use super::*;

    const PRZYKLAD: &str = include_str!("../../przyklady/Zadania.md");

    fn data(t: &str) -> NaiveDate {
        NaiveDate::parse_from_str(t, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parsuje_przyklad_z_b3() {
        let p = parsuj(PRZYKLAD);
        assert_eq!(p.zadania.len(), 4);
        let montaz = &p.zadania[0];
        assert_eq!(montaz.klucz, Klucz::new("Film #1 · FIFA", "Montaż: plansze 1–5"));
        assert_eq!((montaz.szac_min, montaz.czas_min, montaz.zrobione), (Some(120), 45, false));
        assert_eq!(montaz.termin, Some(data("2026-11-05")));
        assert!(montaz.w_toku());
        let miniatura = &p.zadania[1];
        assert_eq!((miniatura.szac_min, miniatura.czas_min, miniatura.termin), (Some(30), 0, None));
        assert!(!miniatura.w_toku());
        let scenariusz = &p.zadania[2];
        assert!(scenariusz.zrobione && !scenariusz.w_toku());
        assert_eq!(scenariusz.klucz.zadanie, "Scenariusz: intro");
        assert_eq!(p.zadania[3].klucz, Klucz::new("Inbox", "Odpisać na maila od montażysty"));
        assert_eq!(p.naglowki.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), ["Film #1 · FIFA", "Inbox"]);
        assert_eq!(p.otwarte().count(), 3);
    }

    #[test]
    fn zadania_przed_pierwszym_naglowkiem_i_podzadania() {
        let p = parsuj("# Zadania\n- [ ] Luźne [szac:: 10]\n\n## A\n- [ ] Główne\n  - [ ] Wcięte podzadanie [szac:: 5]\n    * [x] Jeszcze głębiej\n### Pod-nagłówek\n+ [ ] Plus\n");
        let nazwy: Vec<_> =
            p.zadania.iter().map(|z| (z.klucz.projekt.as_str(), z.klucz.zadanie.as_str(), z.zrobione)).collect();
        assert_eq!(
            nazwy,
            [
                (BEZ_PROJEKTU, "Luźne", false),
                ("A", "Główne", false),
                ("A", "Wcięte podzadanie", false),
                ("A", "Jeszcze głębiej", true),
                ("A", "Plus", false),
            ]
        );
    }

    #[test]
    fn pomija_komentarze_i_bloki_kodu() {
        let tekst = "# Zadania\n<!-- jak pisać:\n- [ ] To nie zadanie [szac:: 1]\n-->\n```\n- [ ] Ani to\n```\n- [ ] Prawdziwe\n<!-- - [ ] jednolinijkowy -->\n";
        let p = parsuj(tekst);
        assert_eq!(p.zadania.len(), 1);
        assert_eq!(p.zadania[0].klucz.zadanie, "Prawdziwe");
    }

    #[test]
    fn klucz_nie_zalezy_od_pol_ani_dat() {
        let a = parsuj("## P\n- [ ] Montaż [szac:: 60] [czas:: 5] [termin:: 2026-11-01]\n");
        let b = parsuj("## P\n- [x] Montaż   [szac:: 90]  ✅ 2026-10-30 📅 2026-11-02\n");
        assert_eq!(a.zadania[0].klucz, b.zadania[0].klucz);
        assert_eq!(tekst_zadania("Tekst #tag [a:: b] ⏳ 2026-01-01"), "Tekst #tag");
    }

    #[test]
    fn dopisuje_czas_w_istniejacym_polu_bez_ruszania_reszty() {
        let k = Klucz::new("Film #1 · FIFA", "Montaż: plansze 1–5");
        let nowy = dopisz_czas(PRZYKLAD, &k, 30, None).unwrap();
        assert_eq!(nowy, PRZYKLAD.replace("[czas:: 45]", "[czas:: 75]"), "zmienia się tylko liczba w polu czas");
        assert_eq!(parsuj(&nowy).zadania[0].czas_min, 75);
    }

    #[test]
    fn zrobione_zaklada_pole_czas_i_dopisuje_ptaszka_z_data() {
        let k = Klucz::new("Film #1 · FIFA", "Miniatura");
        let nowy = dopisz_czas(PRZYKLAD, &k, 40, Some(data("2026-10-30"))).unwrap();
        assert_eq!(
            nowy,
            PRZYKLAD.replace("- [ ] Miniatura [szac:: 30]", "- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30")
        );
        let p = parsuj(&nowy);
        assert!(p.zadania[1].zrobione);
        assert_eq!(p.zadania[1].czas_min, 40);
        // Przerwij: sam czas, bez ptaszka.
        let przerwane = dopisz_czas(PRZYKLAD, &k, 12, None).unwrap();
        assert!(przerwane.contains("- [ ] Miniatura [szac:: 30] [czas:: 12]\n"));
    }

    #[test]
    fn pole_czas_dla_zadania_bez_szacunku_i_z_terminem() {
        let t = "## P\n- [ ] Bez szacunku [termin:: 2026-11-01]\n- [ ] Całkiem goła\n";
        let a = dopisz_czas(t, &Klucz::new("P", "Bez szacunku"), 7, None).unwrap();
        assert!(a.contains("- [ ] Bez szacunku [czas:: 7] [termin:: 2026-11-01]\n"));
        let b = dopisz_czas(t, &Klucz::new("P", "Całkiem goła"), 3, Some(data("2026-10-30"))).unwrap();
        assert!(b.ends_with("- [x] Całkiem goła [czas:: 3] ✅ 2026-10-30\n"));
        // Zero minut i Zrobione: bez pola czas.
        let c = dopisz_czas(t, &Klucz::new("P", "Całkiem goła"), 0, Some(data("2026-10-30"))).unwrap();
        assert!(c.ends_with("- [x] Całkiem goła ✅ 2026-10-30\n"));
    }

    #[test]
    fn zachowuje_crlf_i_brak_konca_pliku() {
        let t = "# Zadania\r\n## P\r\n- [ ] A [szac:: 10]\r\n- [ ] B [szac:: 20]";
        let nowy = dopisz_czas(t, &Klucz::new("P", "B"), 5, None).unwrap();
        assert_eq!(nowy, "# Zadania\r\n## P\r\n- [ ] A [szac:: 10]\r\n- [ ] B [szac:: 20] [czas:: 5]");
        let (z_nowym, k) = dopisz_zadanie(t, "P", "C", Some(15)).unwrap();
        assert_eq!(
            z_nowym,
            "# Zadania\r\n## P\r\n- [ ] A [szac:: 10]\r\n- [ ] B [szac:: 20]\r\n- [ ] C [szac:: 15]\r\n"
        );
        assert_eq!(k, Klucz::new("P", "C"));
    }

    #[test]
    fn nie_znaleziono_gdy_zmieniono_tekst() {
        let k = Klucz::new("Film #1 · FIFA", "Montaż");
        assert!(matches!(dopisz_czas(PRZYKLAD, &k, 5, None), Err(BladEdycji::NieZnaleziono)));
        // Zadanie o tym samym tekście w innym projekcie to inne zadanie.
        assert!(matches!(
            dopisz_czas(PRZYKLAD, &Klucz::new("Inbox", "Miniatura"), 5, None),
            Err(BladEdycji::NieZnaleziono)
        ));
    }

    #[test]
    fn duplikaty_najpierw_otwarte() {
        let t = "## P\n- [x] Powtarzalne [czas:: 10] ✅ 2026-10-01\n- [ ] Powtarzalne [szac:: 20]\n";
        let nowy = dopisz_czas(t, &Klucz::new("P", "Powtarzalne"), 4, None).unwrap();
        assert!(nowy.contains("- [ ] Powtarzalne [szac:: 20] [czas:: 4]\n"));
        assert!(nowy.contains("- [x] Powtarzalne [czas:: 10] ✅"));
    }

    #[test]
    fn dopisuje_zadanie_pod_naglowkiem_po_ostatnim_zadaniu() {
        let (nowy, k) = dopisz_zadanie(PRZYKLAD, "Film #1 · FIFA", "Napisać opis", Some(20)).unwrap();
        assert_eq!(k, Klucz::new("Film #1 · FIFA", "Napisać opis"));
        let oczekiwany = PRZYKLAD.replace("✅ 2026-10-29\n", "✅ 2026-10-29\n- [ ] Napisać opis [szac:: 20]\n");
        assert_eq!(nowy, oczekiwany);
        let p = parsuj(&nowy);
        assert_eq!(p.zadania.iter().filter(|z| z.klucz.projekt == "Film #1 · FIFA").count(), 4);
    }

    #[test]
    fn dopisuje_do_pustego_inboxu_i_zaklada_brakujacy_projekt() {
        let szablon = "# Zadania\n\n## Przykładowy projekt\n- [ ] Pierwsze [szac:: 30]\n\n## Inbox\n";
        let (a, _) = dopisz_zadanie(szablon, INBOX, "Zadzwonić", None).unwrap();
        assert!(a.ends_with("## Inbox\n- [ ] Zadzwonić\n"));
        let (b, k) = dopisz_zadanie("# Zadania\n- [ ] A\n", INBOX, "Nowe", Some(5)).unwrap();
        assert_eq!(b, "# Zadania\n- [ ] A\n\n## Inbox\n- [ ] Nowe [szac:: 5]\n");
        assert_eq!(k.projekt, INBOX);
        let (c, _) = dopisz_zadanie("# Zadania", "Nowy", "X", None).unwrap();
        assert_eq!(c, "# Zadania\n\n## Nowy\n- [ ] X\n");
        assert!(matches!(dopisz_zadanie(szablon, INBOX, "   ", None), Err(BladEdycji::PustaNazwa)));
    }

    #[test]
    fn dopisuje_do_projektu_bez_projektu_przed_pierwszym_naglowkiem() {
        let t = "# Zadania\n- [ ] Luźne\n\n## A\n- [ ] W A\n";
        let (nowy, k) = dopisz_zadanie(t, BEZ_PROJEKTU, "Drugie luźne", None).unwrap();
        assert_eq!(nowy, "# Zadania\n- [ ] Luźne\n- [ ] Drugie luźne\n\n## A\n- [ ] W A\n");
        assert_eq!(k.projekt, BEZ_PROJEKTU);
    }

    #[test]
    fn projekty_do_dopisania_zawsze_maja_inbox() {
        let p = parsuj("## A\n- [ ] x\n## B\n");
        assert_eq!(p.projekty_do_dopisania(), ["A", "B", "Inbox"]);
        let p = parsuj("## Inbox\n## A\n");
        assert_eq!(p.projekty_do_dopisania(), ["Inbox", "A"]);
    }

    #[test]
    fn zmiana_pliku_zachowuje_bom_i_nie_zapisuje_bez_zmian() {
        let k = tempfile::tempdir().unwrap();
        let plik = k.path().join("Zadania.md");
        fs::write(&plik, format!("\u{feff}{PRZYKLAD}")).unwrap();
        let klucz = Klucz::new("Inbox", "Odpisać na maila od montażysty");
        zmien(&plik, |t| Ok((dopisz_czas(t, &klucz, 10, None)?, ()))).unwrap();
        let bajty = fs::read(&plik).unwrap();
        assert_eq!(&bajty[..3], [0xEF, 0xBB, 0xBF]);
        let tekst = String::from_utf8(bajty[3..].to_vec()).unwrap();
        assert_eq!(tekst, PRZYKLAD.replace("[szac:: 15]", "[szac:: 15] [czas:: 10]"));
        // Brak zmiany: plik nie jest dotykany.
        let przed = fs::metadata(&plik).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        zmien(&plik, |t| Ok((t.to_string(), ()))).unwrap();
        assert_eq!(fs::metadata(&plik).unwrap().modified().unwrap(), przed);
        assert_eq!(fs::read_dir(k.path()).unwrap().count(), 1, "plik tymczasowy znika");
    }

    #[test]
    fn plik_nie_w_utf8_zostaje_nietkniety() {
        let k = tempfile::tempdir().unwrap();
        let plik = k.path().join("Zadania.md");
        fs::write(&plik, [0x2D, 0x20, 0x5B, 0x20, 0x5D, 0x20, 0xB1, 0xE6]).unwrap();
        let wynik = zmien(&plik, |t| Ok((t.to_string(), ())));
        assert!(matches!(wynik, Err(BladZmiany::Io(e)) if e.kind() == io::ErrorKind::InvalidData));
        assert_eq!(fs::read(&plik).unwrap().len(), 8);
    }

    #[test]
    fn czyta_od_nowa_tuz_przed_zapisem() {
        // Ty dopisałeś zadanie w Obsidianie po tym, jak widget wczytał plik:
        // zmiana opiera się na świeżej treści, więc dopisane zadanie przeżywa.
        let k = tempfile::tempdir().unwrap();
        let plik = k.path().join("Zadania.md");
        fs::write(&plik, PRZYKLAD).unwrap();
        let wczytany = wczytaj_i_parsuj(&plik).unwrap();
        fs::write(&plik, format!("{PRZYKLAD}- [ ] Dopisane w Obsidianie\n")).unwrap();
        let klucz = wczytany.zadania[0].klucz.clone();
        zmien(&plik, |t| Ok((dopisz_czas(t, &klucz, 5, None)?, ()))).unwrap();
        let po = fs::read_to_string(&plik).unwrap();
        assert!(po.contains("Dopisane w Obsidianie"));
        assert!(po.contains("[czas:: 50]"));
    }

    #[test]
    fn odznacz_zrobione_przywraca_linie_sprzed_zrobione_bajt_w_bajt() {
        let przed = "# Z\n\n## P\n- [ ] Miniatura [szac:: 30] [czas:: 40]\n- [ ] Inne [szac:: 5]\n";
        let po_zrobione = dopisz_czas(przed, &Klucz::new("P", "Miniatura"), 0, Some(data("2026-10-30"))).unwrap();
        assert!(po_zrobione.contains("- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30\n"));
        assert_eq!(odznacz_zrobione(&po_zrobione, &Klucz::new("P", "Miniatura")).unwrap(), przed);
    }

    #[test]
    fn odznacz_zrobione_zachowuje_crlf_i_zadanie_juz_otwarte() {
        let tekst = "## P\r\n- [x] A [szac:: 3] [czas:: 3] ✅ 2026-10-30\r\n- [ ] B\r\n";
        let nowy = odznacz_zrobione(tekst, &Klucz::new("P", "A")).unwrap();
        assert_eq!(nowy, "## P\r\n- [ ] A [szac:: 3] [czas:: 3]\r\n- [ ] B\r\n");
        // odznaczone wcześniej w Obsidianie: nic się nie zmienia
        assert_eq!(odznacz_zrobione(&nowy, &Klucz::new("P", "A")).unwrap(), nowy);
        assert!(matches!(odznacz_zrobione(&nowy, &Klucz::new("P", "Nie ma")), Err(BladEdycji::NieZnaleziono)));
    }

    #[test]
    fn odznacz_zrobione_nie_rusza_innych_linii_ani_tekstu_po_dacie() {
        let tekst = "## P\n- [x] A ✅ 2026-10-29 [szac:: 3]\n- [x] B [czas:: 4] ✅ 2026-10-30\n";
        let nowy = odznacz_zrobione(tekst, &Klucz::new("P", "B")).unwrap();
        assert_eq!(nowy, "## P\n- [x] A ✅ 2026-10-29 [szac:: 3]\n- [ ] B [czas:: 4]\n");
    }
}
