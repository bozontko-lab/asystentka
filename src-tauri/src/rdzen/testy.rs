//! Testy modułu `rdzen`, wydzielone z `rdzen.rs`, żeby kod i testy dało się czytać osobno.

use super::*;
use crate::czas::chwila;
use crate::magazyn::Magazyn;
use std::fs;
use std::path::PathBuf;

const ZADANIA: &str = include_str!("../../../przyklady/Zadania.md");

struct Srodowisko {
    _katalog: tempfile::TempDir,
    vault: PathBuf,
    lokalny: PathBuf,
}

impl Srodowisko {
    fn nowe() -> Srodowisko {
        let katalog = tempfile::tempdir().unwrap();
        let vault = katalog.path().join("vault");
        fs::create_dir_all(&vault).unwrap();
        fs::write(vault.join("Zadania.md"), ZADANIA).unwrap();
        let lokalny = katalog.path().join("AppData").join("Asystentka");
        Srodowisko { vault, lokalny, _katalog: katalog }
    }

    fn plik(&self) -> PathBuf {
        self.vault.join("Zadania.md")
    }

    fn ustawienia(&self) -> Ustawienia {
        Ustawienia {
            imie: "Dżiordżio".into(),
            plik_zadan: self.plik().to_string_lossy().into(),
            folder_dziennika: self.vault.join("Dziennik pracy").to_string_lossy().into(),
            ..Ustawienia::default()
        }
    }

    fn nowy_rdzen(&self, teraz: Chwila) -> Rdzen {
        Rdzen::nowy(Magazyn::new(self.lokalny.clone()), self.ustawienia(), teraz)
    }

    fn rdzen(&self, teraz: Chwila) -> Rdzen {
        let mut r = self.nowy_rdzen(teraz);
        r.start(teraz);
        r
    }

    fn zadania(&self) -> String {
        fs::read_to_string(self.plik()).unwrap()
    }

    fn dziennik(&self, data: &str) -> String {
        fs::read_to_string(self.vault.join("Dziennik pracy").join(format!("{data}.md"))).unwrap()
    }
}

fn t(godzina: &str) -> Chwila {
    chwila(&format!("2026-10-30T{godzina}+01:00"))
}

fn k(projekt: &str, zadanie: &str) -> Klucz {
    Klucz::new(projekt, zadanie)
}

fn montaz() -> Klucz {
    k("Film #1 · FIFA", "Montaż: plansze 1–5")
}

fn miniatura() -> Klucz {
    k("Film #1 · FIFA", "Miniatura")
}

fn mail() -> Klucz {
    k("Inbox", "Odpisać na maila od montażysty")
}

fn dymki(r: &mut Rdzen) -> Vec<String> {
    r.wez_zdarzenia()
        .into_iter()
        .filter_map(|z| match z {
            Zdarzenie::Dymek { klucz, .. } => Some(klucz),
            Zdarzenie::Efekt { .. } => None,
        })
        .collect()
}

fn wstawki(zdarzenia: &[Zdarzenie], klucz_dymku: &str) -> std::collections::BTreeMap<String, String> {
    zdarzenia
        .iter()
        .find_map(|z| match z {
            Zdarzenie::Dymek { klucz, wstawki } if klucz == klucz_dymku => Some(wstawki.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("brak dymku {klucz_dymku}"))
}

/// Powitanie i zatwierdzenie wyboru z domyślnymi zaznaczeniami.
fn zacznij_dzien(r: &mut Rdzen, godzina: &str) {
    r.tick(t(godzina));
    r.tick(t(godzina).max(t(godzina) + Duration::seconds(10)));
    assert_eq!(r.stan(), Stan::WyborZadan);
    r.zatwierdz_wybor(t(godzina));
    assert_eq!(r.stan(), Stan::Lista);
}

// ---- krok 3: wybór zadań ------------------------------------------------------------

#[test]
fn pierwszy_start_dnia_wita_i_pokazuje_wybor_z_zadaniami_w_toku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    assert_eq!(r.stan(), Stan::Powitanie);
    let zdarzenia = r.wez_zdarzenia();
    assert!(zdarzenia.contains(&Zdarzenie::Efekt { nazwa: "glitch_wejscie".into() }));
    assert_eq!(wstawki(&zdarzenia, "powitanie")["imie"], "Dżiordżio");
    r.tick(t("09:00:05"));
    assert_eq!(r.stan(), Stan::Powitanie);
    r.tick(t("09:00:06"));
    assert_eq!(r.stan(), Stan::WyborZadan);
    assert_eq!(dymki(&mut r), vec!["wybor"]);

    let w = r.migawka().wybor.unwrap();
    let nazwy: Vec<(&str, Vec<(&str, bool)>)> = w
        .projekty
        .iter()
        .map(|p| (p.nazwa.as_str(), p.zadania.iter().map(|z| (z.zadanie.as_str(), z.zaznaczone)).collect()))
        .collect();
    assert_eq!(
        nazwy,
        vec![
            ("Film #1 · FIFA", vec![("Montaż: plansze 1–5", true), ("Miniatura", false)]),
            ("Inbox", vec![("Odpisać na maila od montażysty", false)]),
        ],
        "zrobione zadania nie są do wyboru, a zadanie w toku jest zaznaczone z góry"
    );
    // Suma liczy pozostały czas: 120 − 45 min.
    assert_eq!((w.suma_min, w.cel_min, w.przekroczony, w.zaznaczonych), (75, 240, false, 1));
    assert!(!w.mozna_wrocic);
    assert_eq!(r.migawka().projekty_do_dopisania, ["Film #1 · FIFA", "Inbox"]);
}

#[test]
fn restart_tego_samego_dnia_wraca_do_listy_z_tym_samym_planem() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&mail());
    r.zatwierdz_wybor(t("09:01:00"));
    assert_eq!(r.stan(), Stan::Lista);
    r.zapisz_stan(t("09:01:00"));
    drop(r);

    let mut r2 = sr.rdzen(t("11:00:00"));
    assert_eq!(r2.stan(), Stan::Lista);
    assert_eq!(dymki(&mut r2), vec!["powrot"]);
    let lista = r2.migawka().lista.unwrap();
    let nazwy: Vec<&str> = lista.zadania.iter().map(|z| z.zadanie.as_str()).collect();
    assert_eq!(nazwy, ["Montaż: plansze 1–5", "Odpisać na maila od montażysty"]);
    assert_eq!((lista.zrobione, lista.razem), (0, 2));
}

#[test]
fn dzien_bez_planu_po_restarcie_wraca_do_wyboru() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.zapisz_stan(t("09:00:01"));
    drop(r);
    let mut r2 = sr.rdzen(t("09:30:00"));
    assert_eq!(r2.stan(), Stan::WyborZadan, "bez planu nie ma do czego wracać, a powitanie już było");
    assert_eq!(dymki(&mut r2), vec!["wybor"]);
}

#[test]
fn suma_robi_sie_koralowa_po_przekroczeniu_celu_o_20_procent() {
    let sr = Srodowisko::nowe();
    let mut r = sr.nowy_rdzen(t("09:00:00"));
    r.ust.cel_dzienny_min = 95; // próg: 114 min
    r.start(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.wez_zdarzenia();
    r.przelacz_wybor(&miniatura()); // 75 + 30 = 105
    assert!(!r.migawka().wybor.unwrap().przekroczony);
    assert!(dymki(&mut r).is_empty());
    r.przelacz_wybor(&mail()); // 120 > 114
    let w = r.migawka().wybor.unwrap();
    assert_eq!((w.suma_min, w.przekroczony), (120, true));
    assert_eq!(dymki(&mut r), vec!["wybor_za_duzo"]);
    r.przelacz_wybor(&k("Film #1 · FIFA", "Montaż: plansze 1–5")); // zostaje 45
    assert!(!r.migawka().wybor.unwrap().przekroczony);
    r.przelacz_wybor(&montaz());
    assert_eq!(dymki(&mut r), vec!["wybor_za_duzo"], "znów po przekroczeniu progu");
    // Zadania zrobionego nie da się zaznaczyć.
    r.przelacz_wybor(&k("Film #1 · FIFA", "Scenariusz: intro"));
    assert_eq!(r.migawka().wybor.unwrap().zaznaczonych, 3);
}

#[test]
fn suma_liczy_min_10_min_na_zadanie_i_zero_bez_szacunku() {
    let sr = Srodowisko::nowe();
    fs::write(
            sr.plik(),
            "## P\n- [ ] Prawie gotowe [szac:: 30] [czas:: 28]\n- [ ] Przekroczone [szac:: 30] [czas:: 50]\n- [ ] Bez szacunku\n- [ ] Zwykłe [szac:: 40]\n",
        )
        .unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    // Dwa zadania w toku są zaznaczone z góry; dokładam dwa pozostałe.
    for zadanie in ["Bez szacunku", "Zwykłe"] {
        r.przelacz_wybor(&k("P", zadanie));
    }
    let w = r.migawka().wybor.unwrap();
    // 28/30 → 10, 50/30 → 10, bez szacunku → 0, zwykłe → 40.
    assert_eq!(w.zaznaczonych, 4);
    assert_eq!(w.suma_min, 60);
}

#[test]
fn kolejnosc_w_projekcie_zaznaczone_potem_termin_potem_plik() {
    let sr = Srodowisko::nowe();
    fs::write(
            sr.plik(),
            "## P\n- [ ] Pierwsze w pliku\n- [ ] Z późnym terminem [termin:: 2026-12-01]\n- [ ] Z wczesnym terminem [termin:: 2026-11-01]\n- [ ] W toku [czas:: 5] [szac:: 20]\n- [ ] Ostatnie w pliku\n",
        )
        .unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    let w = r.migawka().wybor.unwrap();
    let nazwy: Vec<&str> = w.projekty[0].zadania.iter().map(|z| z.zadanie.as_str()).collect();
    assert_eq!(nazwy, ["W toku", "Z wczesnym terminem", "Z późnym terminem", "Pierwsze w pliku", "Ostatnie w pliku"]);
    assert_eq!(w.projekty[0].zadania[2].termin.as_deref(), Some("2026-12-01"));
}

#[test]
fn niedokonczone_z_poprzedniego_planu_sa_zaznaczone_z_gory() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&montaz()); // odznacz zadanie w toku
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:01:00"));
    r.zapisz_stan(t("09:01:00"));
    drop(r);
    // Następnego dnia Miniatura (bez czasu, więc nie „w toku”) wraca zaznaczona.
    let jutro = chwila("2026-10-31T08:00:00+01:00");
    let mut r2 = sr.rdzen(jutro);
    assert_eq!(r2.stan(), Stan::Powitanie, "nowy dzień ma pełne powitanie");
    r2.tick(jutro + Duration::seconds(10));
    assert_eq!(r2.stan(), Stan::KoniecDnia, "najpierw podsumowanie wczoraj");
    r2.dalej_po_wczoraju();
    assert_eq!(r2.stan(), Stan::WyborZadan);
    let w = r2.migawka().wybor.unwrap();
    let zaznaczone: Vec<&str> =
        w.projekty.iter().flat_map(|p| &p.zadania).filter(|z| z.zaznaczone).map(|z| z.zadanie.as_str()).collect();
    assert_eq!(zaznaczone, ["Montaż: plansze 1–5", "Miniatura"]);
}

#[test]
fn zmien_plan_zachowuje_zrobione_i_pozwala_zmienic_reszte() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:01:00"));
    r.rozpocznij(&miniatura(), t("09:02:00")).unwrap();
    r.zrobione(t("09:50:00"));
    r.tick(t("09:50:10"));
    assert_eq!(r.stan(), Stan::Lista);
    r.otworz_wybor();
    assert_eq!(r.stan(), Stan::WyborZadan);
    let w = r.migawka().wybor.unwrap();
    assert!(w.mozna_wrocic);
    assert_eq!(w.zaznaczonych, 1, "zaznaczony zostaje tylko otwarty Montaż z planu");
    r.przelacz_wybor(&montaz());
    r.przelacz_wybor(&mail());
    r.zatwierdz_wybor(t("09:51:00"));
    let lista = r.migawka().lista.unwrap();
    let nazwy: Vec<(&str, bool)> = lista.zadania.iter().map(|z| (z.zadanie.as_str(), z.zrobione)).collect();
    assert_eq!(nazwy, [("Miniatura", true), ("Odpisać na maila od montażysty", false)]);
    // Anulowanie wraca do listy bez zmian.
    r.otworz_wybor();
    r.przelacz_wybor(&mail());
    r.anuluj_wybor();
    assert_eq!(r.stan(), Stan::Lista);
    assert_eq!(r.migawka().lista.unwrap().zadania.len(), 2);
}

#[test]
fn dodaj_zadanie_dopisuje_linie_i_dodaje_do_planu() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    zacznij_dzien(&mut r, "09:00:00");
    r.dodaj_zadanie("Napisać opis", Some(20), "Film #1 · FIFA").unwrap();
    assert_eq!(sr.zadania(), ZADANIA.replace("✅ 2026-10-29\n", "✅ 2026-10-29\n- [ ] Napisać opis [szac:: 20]\n"));
    let nazwy: Vec<String> = r.migawka().lista.unwrap().zadania.into_iter().map(|z| z.zadanie).collect();
    assert_eq!(nazwy, ["Montaż: plansze 1–5", "Napisać opis"]);
    // Do pustego Inboxu, bez szacunku.
    r.dodaj_zadanie("Zadzwonić", None, "Inbox").unwrap();
    assert!(sr.zadania().contains("- [ ] Odpisać na maila od montażysty [szac:: 15]\n- [ ] Zadzwonić\n"));
    assert!(r.dodaj_zadanie("  ", None, "Inbox").unwrap_err().contains("nazwę"));
}

#[test]
fn dodaj_zadanie_w_wyborze_zaznacza_je_i_liczy_do_sumy() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.dodaj_zadanie("Nowe", Some(30), "Inbox").unwrap();
    let w = r.migawka().wybor.unwrap();
    assert_eq!((w.zaznaczonych, w.suma_min), (2, 105));
    r.zatwierdz_wybor(t("09:01:00"));
    assert_eq!(r.migawka().lista.unwrap().razem, 2);
}

#[test]
fn pusty_plik_zadan_daje_wybor_pusty_i_odswieza_sie_po_dopisaniu() {
    let sr = Srodowisko::nowe();
    fs::write(sr.plik(), "# Zadania\n## Inbox\n").unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    assert_eq!(r.stan(), Stan::WyborZadan);
    assert_eq!(dymki(&mut r), vec!["powitanie", "wybor_pusty"]);
    // Dopisujesz zadanie w Obsidianie: lista pojawia się po odświeżeniu pliku.
    fs::write(sr.plik(), "# Zadania\n## Inbox\n- [ ] Coś [szac:: 20]\n").unwrap();
    assert!(r.odswiez_zadania());
    assert_eq!(r.migawka().wybor.unwrap().projekty[0].zadania.len(), 1);
    assert!(!r.odswiez_zadania(), "bez zmiany nic się nie dzieje");
}

#[test]
fn brak_pliku_zadan_jest_zgloszony_w_migawce() {
    let sr = Srodowisko::nowe();
    fs::remove_file(sr.plik()).unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    assert!(r.migawka().blad_pliku.unwrap().contains("Nie mogę wczytać"));
    fs::write(sr.plik(), ZADANIA).unwrap();
    r.odswiez_zadania();
    assert!(r.migawka().blad_pliku.is_none());
}

#[test]
fn kreator_zastepuje_powitanie_i_prowadzi_do_wyboru() {
    let sr = Srodowisko::nowe();
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
    r.wymus_kreator();
    r.start(t("09:00:00"));
    assert_eq!(r.stan(), Stan::Konfiguracja);
    assert_eq!(dymki(&mut r), vec!["konfiguracja_imie"]);
    r.tick(t("09:00:30"));
    assert_eq!(r.stan(), Stan::Konfiguracja, "kreator czeka na użytkownika");
    r.zakoncz_kreator(sr.ustawienia(), t("09:02:00"));
    assert_eq!(r.stan(), Stan::WyborZadan);
    assert_eq!(dymki(&mut r), vec!["konfiguracja_gotowe"]);
    // Kolejny start tego samego dnia nie wita ponownie.
    r.zapisz_stan(t("09:02:01"));
    let mut r2 = sr.rdzen(t("11:00:00"));
    assert_eq!(r2.stan(), Stan::WyborZadan);
    assert_eq!(dymki(&mut r2), vec!["wybor"]);
}

// ---- sesje, plik zadań i dziennik ---------------------------------------------------------

#[test]
fn dzien_pracy_zapisuje_plik_zadan_i_dziennik() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));

    r.rozpocznij(&miniatura(), t("11:30:00")).unwrap();
    assert!(r.rozpocznij(&montaz(), t("11:31:00")).is_err(), "w Pracy jest zawsze jedno zadanie");
    r.zrobione(t("12:10:00"));
    assert_eq!(r.stan(), Stan::Gratulacje);
    r.tick(t("12:10:10"));
    assert_eq!(r.stan(), Stan::Lista);

    r.rozpocznij(&montaz(), t("10:20:00")).unwrap();
    r.pauza(t("10:40:00"));
    r.wznow(t("10:50:00"));
    r.przerwij(t("11:05:00"));

    assert_eq!(
        sr.zadania(),
        ZADANIA
            .replace("- [ ] Miniatura [szac:: 30]", "- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30")
            .replace("[czas:: 45]", "[czas:: 80]"),
        "Zrobione dopisuje czas, ptaszka i datę; Przerwij tylko czas"
    );
    let dziennik = sr.dziennik("2026-10-30");
    assert!(dziennik.starts_with(
        "---\ndata: 2026-10-30\nminuty: 75\ncel_min: 240\nzaplanowane: 2\nzrobione: 1\n---\n<!-- widget:start -->\n"
    ));
    assert!(dziennik.contains("- [x] 11:30–12:10 · Miniatura · 40 min (szac. 30) · Film #1 · FIFA\n"));
    assert!(dziennik.contains("- [ ] 10:20–11:05 · Montaż: plansze 1–5 · 35 min · w toku · Film #1 · FIFA\n"));
    assert!(dziennik.contains("**Razem:** 1 h 15 min z celu 4 h · **Plan:** 1 z 2 zadań\n"));
    let lista = r.migawka().lista.unwrap();
    assert_eq!((lista.zrobione, lista.razem), (1, 2));
    let montaz = lista.zadania.iter().find(|z| z.zadanie.starts_with("Montaż")).unwrap();
    assert_eq!((montaz.czas_min, montaz.w_toku), (80, true));
}

#[test]
fn licznik_zaczyna_od_czasu_zapisanego_w_pliku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
    let s = r.migawka().sesja.unwrap();
    assert_eq!(s.sekundy_zamkniete, 45 * 60, "licznik pokazuje łączny czas: plik plus bieżąca sesja");
    assert_eq!(s.szac_min, Some(120));
    r.pauza(t("09:30:00"));
    assert_eq!(r.migawka().sesja.unwrap().sekundy_zamkniete, 45 * 60 + 20 * 60);
}

#[test]
fn krotka_przerwana_sesja_nie_trafia_do_pliku_ani_dziennika() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
    r.przerwij(t("09:10:40"));
    assert_eq!(sr.zadania(), ZADANIA);
    assert!(!sr.vault.join("Dziennik pracy").exists());
}

#[test]
fn zadanie_ze_zmienionym_tekstem_zostaje_tylko_w_dzienniku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&miniatura(), t("09:10:00")).unwrap();
    // Zmieniasz tekst zadania w Obsidianie w trakcie sesji.
    fs::write(sr.plik(), ZADANIA.replace("Miniatura [szac:: 30]", "Miniatura v2 [szac:: 30]")).unwrap();
    r.wez_zdarzenia();
    r.zrobione(t("09:50:00"));
    assert_eq!(sr.zadania(), ZADANIA.replace("Miniatura [szac:: 30]", "Miniatura v2 [szac:: 30]"));
    let zd = r.wez_zdarzenia();
    assert_eq!(wstawki(&zd, "nie_znaleziono")["zadanie"], "Miniatura");
    assert!(sr.dziennik("2026-10-30").contains("- [x] 09:10–09:50 · Miniatura · 40 min (szac. 30)"));
}

#[test]
fn niedostepny_plik_zadan_zapisy_czekaja_w_kolejce() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
    let schowany = sr.plik().with_extension("offline");
    fs::rename(sr.plik(), &schowany).unwrap();
    r.przerwij(t("10:10:00"));
    assert_eq!(r.dz.oczekujace.len(), 1);
    assert!(sr.dziennik("2026-10-30").contains("60 min · w toku"), "dziennik jest w vaulcie i zapisuje się od razu");
    fs::rename(&schowany, sr.plik()).unwrap();
    r.cykl(t("10:10:30"));
    assert!(r.dz.oczekujace.is_empty());
    assert!(sr.zadania().contains("[czas:: 105]"));
}

#[test]
fn po_awarii_sesja_wraca_zapauzowana_bez_przestoju() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:00:00")).unwrap();
    r.zapisz_stan(t("09:00:00"));
    r.cykl(t("09:20:00"));
    r.cykl(t("09:20:30"));
    drop(r); // proces zabity bez sprzątania
    let mut r = sr.rdzen(t("10:30:00"));
    assert_eq!(r.stan(), Stan::Pauza);
    assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
    let s = r.dz.sesja.as_ref().unwrap();
    assert_eq!(s.sekundy(t("10:30:00")), 20 * 60 + 30);
    assert_eq!(s.pauza.as_ref().unwrap().powod, PowodPauzy::Awaria);
    assert_eq!(r.migawka().sesja.unwrap().sekundy_zamkniete, (45 * 60) + 20 * 60 + 30);
}

#[test]
fn nowy_dzien_zamyka_zapauzowana_sesje_z_wczoraj_jako_w_toku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("18:00:00"));
    r.tick(t("18:00:06"));
    r.zatwierdz_wybor(t("18:00:10"));
    r.rozpocznij(&montaz(), t("18:00:00")).unwrap();
    r.pauza(t("18:45:00"));
    r.zapisz_stan(t("18:45:00"));
    drop(r);
    let mut r = sr.rdzen(chwila("2026-10-31T09:00:00+01:00"));
    assert!(r.dz.sesja.is_none());
    assert_eq!(r.stan(), Stan::Powitanie);
    assert!(sr.dziennik("2026-10-30").contains("- [ ] 18:00–18:45 · Montaż: plansze 1–5 · 45 min · w toku"));
    assert!(sr.zadania().contains("[czas:: 90]"));
    assert!(dymki(&mut r).contains(&"powitanie".to_string()));
}

#[test]
fn sesja_przez_polnoc_nalezy_do_dnia_startu() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("23:00:00"));
    r.tick(t("23:00:06"));
    r.zatwierdz_wybor(t("23:00:10"));
    r.rozpocznij(&montaz(), t("23:30:00")).unwrap();
    r.cykl(chwila("2026-10-31T00:20:00+01:00"));
    assert_eq!(r.stan(), Stan::Praca, "trwająca sesja liczy się dalej");
    r.zrobione(chwila("2026-10-31T00:30:00+01:00"));
    assert!(sr.dziennik("2026-10-30").contains("- [x] 23:30–00:30 · Montaż: plansze 1–5 · 60 min (szac. 120)"));
    assert!(sr.zadania().contains("✅ 2026-10-30"), "ptaszek ma datę dnia, w którym zaczęła się sesja");
    // Po sesji dzień się zmienia i otwiera się wybór nowego dnia.
    r.tick(chwila("2026-10-31T00:30:10+01:00"));
    assert_eq!(r.stan(), Stan::KoniecDnia, "dzień, którego nie zamknąłeś, zamyka się za Ciebie z podsumowaniem");
    assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
    r.dalej_po_wczoraju();
    assert_eq!(r.stan(), Stan::WyborZadan);
}

#[test]
fn zrobione_po_polnocy_zostawia_gratulacje_i_cofnij_na_10_s() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("23:00:00"));
    r.tick(t("23:00:06"));
    r.zatwierdz_wybor(t("23:00:10"));
    r.rozpocznij(&montaz(), t("23:30:00")).unwrap();
    let zrobione = chwila("2026-10-31T00:30:00+01:00");
    r.zrobione(zrobione);
    // Wątek terminów nie może uznać północy za zaległy termin w trakcie gratulacji.
    assert_eq!(r.najblizszy_termin(zrobione), chwila("2026-10-31T00:30:10+01:00"));
    // Tyknięcie 20 ms później, jak po obudzeniu wątku przez `wykonaj`.
    r.tick(zrobione + Duration::milliseconds(20));
    assert_eq!(r.stan(), Stan::Gratulacje, "zmiana dnia czeka na koniec gratulacji");
    assert!(r.migawka().gratulacje.unwrap().mozna_cofnac);
    r.cofnij(chwila("2026-10-31T00:30:05+01:00"));
    assert_eq!(r.stan(), Stan::Praca, "Cofnij działa także po północy");
    assert_eq!(r.migawka().sesja.unwrap().zadanie, montaz().zadanie);
    assert!(!sr.zadania().contains("✅ 2026-10-30"), "ptaszek zdjęty");
}

/// Jak wątek terminów (`Aplikacja::petla`): gdy termin minął, woła `tick`. Po obrocie
/// najbliższy termin musi być w przyszłości, inaczej wątek budziłby się co 20 ms.
#[test]
fn po_tyknieciu_termin_jest_zawsze_w_przyszlosci_takze_po_22_i_po_polnocy() {
    for (start, wroc_do_pracy) in [("21:30:00", false), ("21:59:00", false), ("21:30:00", true)] {
        let sr = Srodowisko::nowe();
        let mut r = sr.rdzen(t("21:00:00"));
        r.tick(t("21:00:06"));
        r.zatwierdz_wybor(t("21:00:10"));
        r.rozpocznij(&montaz(), t(start)).unwrap();
        let mut teraz = t("21:00:30");
        let koniec = chwila("2026-10-31T02:00:00+01:00");
        while teraz < koniec {
            if r.najblizszy_termin(teraz) <= teraz {
                r.tick(teraz);
            }
            assert!(
                r.najblizszy_termin(teraz) > teraz,
                "start {start}, wróć do pracy {wroc_do_pracy}: zaległy termin o {teraz} w stanie {:?}",
                r.stan()
            );
            if wroc_do_pracy && r.stan() == Stan::KoniecDnia {
                r.wroc_do_pracy(teraz);
                r.rozpocznij(&montaz(), teraz).unwrap();
            }
            teraz += Duration::seconds(37);
        }
    }
}

#[test]
fn po_gratulacjach_po_polnocy_dzien_sie_zmienia() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("23:00:00"));
    r.tick(t("23:00:06"));
    r.zatwierdz_wybor(t("23:00:10"));
    r.rozpocznij(&montaz(), t("23:30:00")).unwrap();
    r.zrobione(chwila("2026-10-31T00:30:00+01:00"));
    r.tick(chwila("2026-10-31T00:30:09+01:00"));
    assert_eq!(r.stan(), Stan::Gratulacje);
    r.tick(chwila("2026-10-31T00:30:10+01:00"));
    assert_eq!(r.stan(), Stan::KoniecDnia, "po 10 s wczorajszy dzień zamyka się z podsumowaniem");
    assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
}

// ---- krok 4: podsumowanie po zadaniu --------------------------------------------------------

#[test]
fn po_zadaniu_dymek_pokazuje_czas_zadania_szacunek_i_czas_dnia() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));
    // Miniatura: 40 min; Montaż ma już 45 min w pliku i dochodzi 30 min.
    r.rozpocznij(&miniatura(), t("10:00:00")).unwrap();
    r.wez_zdarzenia();
    r.zrobione(t("10:40:00"));
    let zd = r.wez_zdarzenia();
    let w = wstawki(&zd, "zrobione");
    assert_eq!(w["zadanie"], "Miniatura");
    assert_eq!(w["czas"], "40 min");
    assert_eq!(w["szac"], "30 min");
    assert_eq!(w["czas_dnia"], "40 min");
    assert!(zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti".into() }));
    let g = r.migawka().gratulacje.unwrap();
    assert_eq!((g.czas_min, g.szac_min, g.czas_dnia_min, g.plan_wykonany), (40, Some(30), 40, false));

    // Drugie zadanie tego samego dnia: łączny czas zadania z pliku i czas całego dnia.
    r.tick(t("10:40:10"));
    assert_eq!(dymki(&mut r), vec!["zostalo"]);
    r.rozpocznij(&montaz(), t("11:00:00")).unwrap();
    r.zrobione(t("11:30:00"));
    let w = wstawki(&r.wez_zdarzenia(), "zrobione");
    assert_eq!(w["czas"], "1 h 15 min", "45 min z pliku plus 30 min tej sesji");
    assert_eq!(w["szac"], "2 h");
    assert_eq!(w["czas_dnia"], "1 h 10 min");
}

#[test]
fn zadanie_bez_szacunku_pomija_szacunek_w_dymku() {
    let sr = Srodowisko::nowe();
    fs::write(
        sr.plik(),
        "## P
- [ ] Bez szacunku
- [ ] Drugie [szac:: 20]
",
    )
    .unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&k("P", "Bez szacunku"));
    r.przelacz_wybor(&k("P", "Drugie"));
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&k("P", "Bez szacunku"), t("10:00:00")).unwrap();
    r.wez_zdarzenia();
    r.zrobione(t("10:10:00"));
    let zd = r.wez_zdarzenia();
    assert!(!wstawki(&zd, "zrobione").contains_key("szac"));
    assert_eq!(r.migawka().gratulacje.unwrap().szac_min, None);
}

#[test]
fn zostalo_liczy_zadania_i_pozostaly_czas() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.przelacz_wybor(&mail());
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&mail(), t("10:00:00")).unwrap();
    r.zrobione(t("10:10:00"));
    r.tick(t("10:10:10"));
    let zd = r.wez_zdarzenia();
    let w = wstawki(&zd, "zostalo");
    // Zostają Montaż (120 − 45) i Miniatura (30): 2 zadania, 1 h 45 min.
    assert_eq!((w["zadan"].as_str(), w["pozostalo"].as_str()), ("2 zadania", "1 h 45 min"));
    assert_eq!(r.migawka().lista.unwrap().pozostalo_min, 105);
}

#[test]
fn po_ostatnim_zadaniu_planu_postac_pyta_czy_konczymy() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&montaz()); // tylko Miniatura
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&miniatura(), t("10:00:00")).unwrap();
    r.zrobione(t("10:40:00"));
    assert!(r.migawka().gratulacje.unwrap().plan_wykonany);
    r.wez_zdarzenia();
    r.tick(t("10:40:10"));
    assert_eq!(r.stan(), Stan::Gratulacje, "po ostatnim zadaniu to Ty wybierasz, co dalej");
    assert_eq!(dymki(&mut r), vec!["plan_wykonany"]);
    r.tick(t("10:50:00"));
    assert_eq!(r.stan(), Stan::Gratulacje, "bez automatycznego powrotu do listy");
    assert!(r.najblizszy_termin(t("10:50:00")) > t("10:50:00"));
    // „Dodaj z listy”: ten sam wybór z planem jako punktem wyjścia.
    r.otworz_wybor();
    assert_eq!(r.stan(), Stan::WyborZadan);
    r.przelacz_wybor(&mail());
    r.zatwierdz_wybor(t("10:51:00"));
    let lista = r.migawka().lista.unwrap();
    assert_eq!((lista.zrobione, lista.razem), (1, 2));
}

#[test]
fn zadanie_dopisane_w_ciagu_dnia_wydluza_plan() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&montaz(), t("10:00:00")).unwrap();
    r.dodaj_zadanie("Pilne", Some(10), "Inbox").unwrap();
    r.zrobione(t("10:30:00"));
    assert!(!r.migawka().gratulacje.unwrap().plan_wykonany, "dopisane w trakcie pracy zadanie liczy się do planu");
}

// ---- krok 5: koniec dnia -------------------------------------------------------------------

/// Sesja `od`–`do_` na zadaniu: Zrobione albo Przerwij.
fn sesja(r: &mut Rdzen, klucz: &Klucz, od: &str, do_: &str, zrobione: bool) {
    r.rozpocznij(klucz, t(od)).unwrap();
    if zrobione {
        r.zrobione(t(do_));
    } else {
        r.przerwij(t(do_));
    }
}

/// Plan z jednym zadaniem i dzień z sesją o podanej długości.
fn dzien_z(sr: &Srodowisko, minuty: u32, zrobione: bool) -> Rdzen {
    let mut r = sr.rdzen(t("08:00:00"));
    r.tick(t("08:00:06"));
    r.przelacz_wybor(&montaz()); // tylko Miniatura
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("08:00:30"));
    if minuty > 0 {
        let koniec = chwila("2026-10-30T08:10:00+01:00") + Duration::minutes(i64::from(minuty));
        let koniec = koniec.format("%H:%M:%S").to_string();
        sesja(&mut r, &miniatura(), "08:10:00", &koniec, zrobione);
    }
    r
}

#[test]
fn szesc_reakcji_na_koniec_dnia_z_tabeli_b4() {
    // (minuty, zrobione, reakcja, dymek, nazwa w dzienniku)
    let przypadki = [
        (250, true, Reakcja::Zachwycona, "koniec_zachwycona", "zachwycona"),
        (250, false, Reakcja::Zadowolona, "koniec_zadowolona", "zadowolona"),
        (120, true, Reakcja::Dumna, "koniec_dumna", "dumna"),
        (120, false, Reakcja::LekkoPochwalajaca, "koniec_pochwala", "lekko pochwalająca"),
        (30, true, Reakcja::Czula, "koniec_czula", "czuła"),
        (0, false, Reakcja::Zmartwiona, "koniec_zmartwiona", "zmartwiona"),
    ];
    for (minuty, zrobione, reakcja, dymek, nazwa) in przypadki {
        let sr = Srodowisko::nowe();
        let mut r = dzien_z(&sr, minuty, zrobione);
        r.wez_zdarzenia();
        r.koniec_pracy(chwila("2026-10-30T14:00:00+01:00"));
        let zd = r.wez_zdarzenia();
        assert_eq!(wstawki(&zd, dymek)["imie"], "Dżiordżio", "{nazwa}");
        let k = r.migawka().koniec.unwrap();
        assert_eq!((k.etap, k.reakcja), ("reakcja", reakcja), "{nazwa}");
        let dz = sr.dziennik("2026-10-30");
        assert!(dz.contains(&format!("### Podsumowanie dnia\nReakcja: {nazwa}\n")), "{nazwa}: {dz}");
        // Zachwycona ma konfetti 3 s, dumna 1,5 s.
        let konfetti_dlugie = zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti_dlugie".into() });
        let konfetti = zd.contains(&Zdarzenie::Efekt { nazwa: "konfetti".into() });
        assert_eq!((konfetti_dlugie, konfetti), (reakcja == Reakcja::Zachwycona, reakcja == Reakcja::Dumna), "{nazwa}");
    }
}

#[test]
fn podsumowanie_dnia_ma_zrobione_czas_zadan_i_liste_na_jutro() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("08:00:00"));
    r.tick(t("08:00:06"));
    r.przelacz_wybor(&miniatura());
    r.przelacz_wybor(&mail());
    r.zatwierdz_wybor(t("08:00:30"));
    sesja(&mut r, &miniatura(), "08:10:00", "08:50:00", true);
    sesja(&mut r, &montaz(), "09:00:00", "09:30:00", false);
    sesja(&mut r, &montaz(), "10:00:00", "10:15:00", false);
    r.koniec_pracy(t("12:00:00"));
    let p = r.migawka().koniec.unwrap().podsumowanie;
    assert_eq!((p.zrobione, p.zaplanowane, p.czas_dnia_min, p.cel_min), (1, 3, 85, 240));
    let zadania: Vec<(&str, u32, Option<u32>, bool)> =
        p.zadania.iter().map(|z| (z.zadanie.as_str(), z.czas_min, z.szac_min, z.zrobione)).collect();
    assert_eq!(
        zadania,
        [("Miniatura", 40, Some(30), true), ("Montaż: plansze 1–5", 45, Some(120), false)],
        "kolejne sesje tego samego zadania sumują się"
    );
    assert_eq!(p.na_jutro, ["Montaż: plansze 1–5", "Odpisać na maila od montażysty"]);
    let dz = sr.dziennik("2026-10-30");
    assert!(dz.contains("**Razem:** 1 h 25 min z celu 4 h · **Plan:** 1 z 3 zadań\n"));
    assert!(dz.contains("Na jutro: Montaż: plansze 1–5, Odpisać na maila od montażysty\n"));
}

#[test]
fn koniec_pracy_przy_dzialajacym_liczniku_zapisuje_sesje_jak_przerwij() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("08:00:00"));
    r.tick(t("08:00:06"));
    r.zatwierdz_wybor(t("08:00:10"));
    r.rozpocznij(&montaz(), t("08:10:00")).unwrap();
    r.koniec_pracy(t("09:10:00"));
    assert!(r.dz.sesja.is_none());
    assert_eq!(r.stan(), Stan::KoniecDnia);
    assert!(sr.zadania().contains("[czas:: 105]"), "60 min dopisane do 45");
    assert!(!sr.zadania().contains("[x] Montaż"), "bez ptaszka");
    assert!(sr.dziennik("2026-10-30").contains("- [ ] 08:10–09:10 · Montaż: plansze 1–5 · 60 min · w toku"));
}

#[test]
fn po_reakcji_postac_macha_i_chowa_sie_do_zasobnika() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("14:00:00"));
    r.wez_zdarzenia();
    r.tick(t("14:00:07"));
    assert_eq!(r.migawka().koniec.unwrap().etap, "reakcja");
    assert!(r.widoczna());
    r.tick(t("14:00:08"));
    assert_eq!(r.migawka().koniec.unwrap().etap, "pozegnanie");
    assert_eq!(dymki(&mut r), vec!["do_jutra"]);
    r.tick(t("14:00:11"));
    assert_eq!(r.migawka().koniec.unwrap().etap, "spi");
    assert!(!r.widoczna(), "okno chowa się do zasobnika");
    // Pokazane z zasobnika: „śpi” z podsumowaniem i przyciskiem „Wróć do pracy”.
    r.pokaz();
    assert!(r.widoczna());
    assert_eq!(r.migawka().koniec.unwrap().etap, "spi");
    r.do_jutra(t("14:30:00"));
    assert!(!r.widoczna());
}

#[test]
fn do_jutra_skraca_reakcje() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("14:00:00"));
    r.wez_zdarzenia();
    r.do_jutra(t("14:00:02"));
    assert_eq!(r.migawka().koniec.unwrap().etap, "pozegnanie");
    assert_eq!(dymki(&mut r), vec!["do_jutra"]);
}

#[test]
fn wroc_do_pracy_otwiera_dzien_i_usuwa_podsumowanie_z_dziennika() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("14:00:00"));
    assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
    r.wez_zdarzenia();
    r.wroc_do_pracy(t("14:01:00"));
    assert_eq!(r.stan(), Stan::Lista);
    assert!(!sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
    assert!(sr.dziennik("2026-10-30").contains("- [x] 08:10–08:40 · Miniatura"), "sesje zostają");
    assert!(!r.dz.zamkniety);
    // Dzień można zamknąć ponownie, a podsumowanie powstaje od nowa.
    r.koniec_pracy(t("15:00:00"));
    assert!(sr.dziennik("2026-10-30").contains("Reakcja: czuła"));
    assert!(r.rozpocznij(&montaz(), t("15:01:00")).is_err(), "w zamkniętym dniu nie ma pracy");
    assert!(r.dodaj_zadanie("X", None, "Inbox").is_err());
}

#[test]
fn o_22_dzien_zamyka_sie_sam_nawet_przy_dzialajacym_liczniku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("20:00:00"));
    r.tick(t("20:00:06"));
    r.zatwierdz_wybor(t("20:00:10"));
    r.rozpocznij(&montaz(), t("21:30:00")).unwrap();
    assert_eq!(r.najblizszy_termin(t("21:31:00")), t("22:00:00"), "zamknięcie dnia przed przypomnieniem o przerwie");
    r.tick(t("21:59:59"));
    assert_eq!(r.stan(), Stan::Praca);
    r.tick(t("22:00:00"));
    assert_eq!(r.stan(), Stan::KoniecDnia);
    assert!(sr.dziennik("2026-10-30").contains("- [ ] 21:30–22:00 · Montaż: plansze 1–5 · 30 min · w toku"));
    assert!(sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
    // Po zamknięciu nie ma już terminu na 22:00.
    r.tick(t("22:00:20"));
    assert!(r.najblizszy_termin(t("22:00:20")) > t("22:00:20"));
}

#[test]
fn dzien_bez_aktywnosci_nie_zamyka_sie_sam() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("20:00:00"));
    r.tick(t("20:00:06"));
    assert_eq!(r.stan(), Stan::WyborZadan);
    r.tick(t("22:05:00"));
    assert_eq!(r.stan(), Stan::WyborZadan);
    // Plan wybrany po godzinie zamknięcia to Twoja decyzja: dzień nie zamyka się sam.
    r.zatwierdz_wybor(t("22:10:00"));
    r.tick(t("22:11:00"));
    assert_eq!(r.stan(), Stan::Lista);
    r.rozpocznij(&montaz(), t("22:12:00")).unwrap();
    r.tick(t("23:00:00"));
    assert_eq!(r.stan(), Stan::Praca);
}

#[test]
fn po_wroc_do_pracy_dzien_nie_zamyka_sie_ponownie_o_22() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("21:00:00"));
    r.wroc_do_pracy(t("21:30:00"));
    r.tick(t("22:00:00"));
    assert_eq!(r.stan(), Stan::Lista);
}

#[test]
fn komputer_wlaczony_po_22_zamyka_dzien_przy_starcie() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 120, false);
    r.zapisz_stan(t("11:00:00"));
    drop(r);
    let mut r2 = sr.rdzen(t("23:10:00"));
    assert_eq!(r2.stan(), Stan::Lista, "start pokazuje to, co było");
    r2.tick(t("23:10:01"));
    assert_eq!(r2.stan(), Stan::KoniecDnia);
    assert_eq!(r2.migawka().koniec.unwrap().reakcja, Reakcja::LekkoPochwalajaca);
}

#[test]
fn restart_w_zamknietym_dniu_pokazuje_podsumowanie_i_wroc_do_pracy() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("14:00:00"));
    drop(r);
    let r2 = sr.rdzen(t("15:00:00"));
    assert_eq!(r2.stan(), Stan::KoniecDnia);
    let k = r2.migawka().koniec.unwrap();
    assert_eq!((k.etap, k.reakcja, k.podsumowanie.czas_dnia_min), ("spi", Reakcja::Czula, 30));
}

#[test]
fn niezamkniety_wczorajszy_dzien_zamyka_sie_za_ciebie_i_pokazuje_podsumowanie() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 120, false);
    r.zapisz_stan(t("11:00:00"));
    drop(r);
    let jutro = chwila("2026-10-31T08:00:00+01:00");
    let mut r2 = sr.rdzen(jutro);
    assert_eq!(r2.stan(), Stan::Powitanie);
    assert!(sr.dziennik("2026-10-30").contains("Reakcja: lekko pochwalająca"), "wczorajszy dziennik zamknięty");
    r2.wez_zdarzenia();
    r2.tick(jutro + Duration::seconds(10));
    let k = r2.migawka().koniec.unwrap();
    assert_eq!((k.etap, k.reakcja, k.podsumowanie.czas_dnia_min), ("wczoraj", Reakcja::LekkoPochwalajaca, 120));
    assert_eq!(dymki(&mut r2), vec!["zamkniety_wczoraj"]);
    // „Dalej”: wybór zadań na dziś, z niedokończonymi zadaniami z wczoraj.
    r2.dalej_po_wczoraju();
    assert_eq!(r2.stan(), Stan::WyborZadan);
    assert!(r2.migawka().wybor.unwrap().zaznaczonych >= 1);
}

#[test]
fn dzien_zamkniety_przez_ciebie_nie_pokazuje_wczoraj_ponownie() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("14:00:00"));
    drop(r);
    let jutro = chwila("2026-10-31T08:00:00+01:00");
    let mut r2 = sr.rdzen(jutro);
    r2.tick(jutro + Duration::seconds(10));
    assert_eq!(r2.stan(), Stan::WyborZadan);
}

#[test]
fn polnoc_w_zamknietym_dniu_otwiera_wybor_nowego_dnia() {
    let sr = Srodowisko::nowe();
    let mut r = dzien_z(&sr, 30, true);
    r.koniec_pracy(t("23:00:00"));
    r.tick(t("23:00:20"));
    r.tick(chwila("2026-10-31T00:00:05+01:00"));
    assert_eq!(r.stan(), Stan::WyborZadan);
}

#[test]
fn nie_zamyka_dnia_w_kreatorze() {
    let sr = Srodowisko::nowe();
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("23:00:00"));
    r.wymus_kreator();
    r.start(t("23:00:00"));
    r.koniec_pracy(t("23:00:10"));
    r.tick(t("23:30:00"));
    assert_eq!(r.stan(), Stan::Konfiguracja);
}

// ---- krok 6: menu i zamykanie ---------------------------------------------------------------

#[test]
fn menu_zalezy_od_stanu() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    let wybor = r.mozliwosci();
    assert_eq!((wybor.koniec_pracy, wybor.zmien_plan, wybor.dodaj_zadanie, wybor.pauza), (true, false, true, None));
    r.zatwierdz_wybor(t("09:00:10"));
    let lista = r.mozliwosci();
    assert_eq!((lista.koniec_pracy, lista.zmien_plan, lista.dodaj_zadanie), (true, true, true));
    r.rozpocznij(&montaz(), t("09:01:00")).unwrap();
    let praca = r.mozliwosci();
    assert_eq!(
        (praca.koniec_pracy, praca.zmien_plan, praca.dodaj_zadanie, praca.pauza),
        (true, false, true, Some(false))
    );
    r.pauza(t("09:02:00"));
    assert_eq!(r.mozliwosci().pauza, Some(true));
    r.koniec_pracy(t("09:03:00"));
    let koniec = r.mozliwosci();
    assert_eq!((koniec.koniec_pracy, koniec.zmien_plan, koniec.dodaj_zadanie), (false, false, false));
    assert!(koniec.widoczna);
}

#[test]
fn kreator_nie_ma_menu_dnia() {
    let sr = Srodowisko::nowe();
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
    r.wymus_kreator();
    r.start(t("09:00:00"));
    let m = r.mozliwosci();
    assert_eq!((m.koniec_pracy, m.zmien_plan, m.dodaj_zadanie), (false, false, false));
}

#[test]
fn zamknij_asystentke_zapisuje_sesje_i_zostawia_dzien_otwarty() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:10:00")).unwrap();
    r.zamknij_asystentke(t("09:40:00"));
    assert!(r.dz.sesja.is_none());
    assert!(sr.zadania().contains("[czas:: 75]"), "30 min dopisane do 45");
    assert!(!sr.zadania().contains("[x] Montaż"), "bez ptaszka");
    assert!(sr.dziennik("2026-10-30").contains("- [ ] 09:10–09:40 · Montaż: plansze 1–5 · 30 min · w toku"));
    assert!(!sr.dziennik("2026-10-30").contains("Podsumowanie dnia"));
    drop(r);
    // Kolejne uruchomienie tego samego dnia wraca do Listy z tym samym planem.
    let mut r2 = sr.rdzen(t("10:00:00"));
    assert_eq!(r2.stan(), Stan::Lista);
    assert_eq!(dymki(&mut r2), vec!["powrot"]);
    assert!(r2.dz.sesja.is_none());
    assert_eq!(r2.migawka().lista.unwrap().razem, 1);
}

#[test]
fn zamknij_asystentke_bez_sesji_tylko_zapisuje_stan() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.zamknij_asystentke(t("09:05:00"));
    assert!(sr.lokalny.join("dzien.json").exists());
    assert!(!sr.vault.join("Dziennik pracy").exists());
}

// ---- bezczynność, pauza automatyczna, pełny ekran (z wersji 1) -------------------------------

fn w_pracy(sr: &Srodowisko) -> Rdzen {
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&montaz(), t("09:00:00")).unwrap();
    r.wez_zdarzenia();
    r
}

#[test]
fn bezczynnosc_pyta_i_odejmuje_czas() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    r.bezczynnosc(14 * 60, t("09:30:00"));
    assert_eq!(r.stan(), Stan::Praca);
    r.bezczynnosc(16 * 60, t("09:46:00"));
    assert_eq!(r.stan(), Stan::Jestes);
    assert_eq!(dymki(&mut r), vec!["jestes"]);
    r.odpowiedz_jestes(true, t("09:50:00"));
    assert_eq!(r.stan(), Stan::Praca);
    // 09:00–09:30 praca, 09:30–09:50 przerwa, potem 10 min pracy.
    assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("10:00:00")), 40 * 60);
}

#[test]
fn pracowalem_zostawia_czas_i_brak_pytania_przy_pelnym_ekranie() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    r.ustaw_pelny_ekran(true);
    assert!(!r.widoczna());
    r.bezczynnosc(20 * 60, t("09:30:00"));
    assert_eq!(r.stan(), Stan::Praca, "schowany przez pełny ekran nie pyta");
    r.ustaw_pelny_ekran(false);
    r.bezczynnosc(20 * 60, t("09:31:00"));
    r.odpowiedz_jestes(false, t("09:32:00"));
    assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("09:40:00")), 40 * 60);
}

#[test]
fn pelny_ekran_na_innym_monitorze_nie_chowa_widgetu_ale_blokuje_pytanie() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    // Film na drugim monitorze: monitor widgetu bez pełnego ekranu.
    r.ustaw_pelny_ekran(false);
    r.ustaw_pelny_ekran_gdziekolwiek(true);
    assert!(r.widoczna(), "chowanie liczy się tylko na monitorze widgetu (B6)");
    r.bezczynnosc(20 * 60, t("09:30:00"));
    assert_eq!(r.stan(), Stan::Praca, "pełny ekran na którymkolwiek monitorze: bez „Jesteś?” (B5)");
    r.ustaw_pelny_ekran_gdziekolwiek(false);
    r.bezczynnosc(20 * 60, t("09:31:00"));
    assert_eq!(r.stan(), Stan::Jestes);
}

#[test]
fn pelny_ekran_bez_chowania_tez_blokuje_pytanie() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    let ust = r.ust.clone();
    r.zmien_ustawienia(Ustawienia { chowaj_przy_pelnym_ekranie: false, ..ust }, t("09:10:00"));
    r.ustaw_pelny_ekran(true);
    r.ustaw_pelny_ekran_gdziekolwiek(true);
    assert!(r.widoczna(), "chowanie wyłączone w ustawieniach");
    r.bezczynnosc(20 * 60, t("09:30:00"));
    assert_eq!(r.stan(), Stan::Praca);
}

#[test]
fn migawka_niesie_wysokosc_monitora_widgetu() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    assert_eq!(r.migawka().monitor_wys, None, "przed ustawieniem okna frontend bierze ekran");
    r.ustaw_monitor_wys(Some(693));
    assert_eq!(r.migawka().monitor_wys, Some(693));
    r.ustaw_monitor_wys(Some(1880));
    assert_eq!(r.migawka().monitor_wys, Some(1880), "przeciągnięcie na pionowy monitor");
}

#[test]
fn blokada_i_uspienie_pauzuja() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    r.pauza_automatyczna(PowodPauzy::Blokada, t("09:20:00"));
    assert_eq!(r.stan(), Stan::Pauza);
    r.powrot(t("09:45:00"));
    assert_eq!(r.stan(), Stan::Pauza, "po powrocie pauza zostaje");
    assert_eq!(dymki(&mut r), vec!["po_blokadzie"]);
    r.powrot(t("09:45:02"));
    assert!(dymki(&mut r).is_empty());
    r.wznow(t("09:46:00"));
    r.pauza_automatyczna(PowodPauzy::Uspienie, t("10:00:00"));
    assert_eq!(r.dz.sesja.as_ref().unwrap().sekundy(t("12:00:00")), 34 * 60);
}

#[test]
fn przypomnienie_o_przerwie_po_50_min_ciaglej_pracy() {
    let sr = Srodowisko::nowe();
    let mut r = w_pracy(&sr);
    assert_eq!(r.najblizszy_termin(t("09:10:00")), t("09:50:00"));
    r.tick(t("09:49:59"));
    assert!(dymki(&mut r).is_empty());
    r.tick(t("09:50:00"));
    let zd = r.wez_zdarzenia();
    assert_eq!(wstawki(&zd, "przypomnienie_przerwy")["minuty"], "50 min");
    assert!(zd.contains(&Zdarzenie::Efekt { nazwa: "przeciaganie".into() }));
    r.tick(t("09:55:00"));
    assert!(r.wez_zdarzenia().is_empty());
}

#[test]
fn zamkniecie_okna_chowa_i_nie_pokazuje_schowanego() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.ukryj_recznie();
    assert!(!r.widoczna(), "Alt+F4 chowa widget");
    r.ukryj_recznie();
    assert!(!r.widoczna(), "drugie zamknięcie schowanego okna go nie pokazuje");
    r.ustaw_pelny_ekran(true);
    r.pokaz();
    assert!(r.widoczna());
    r.ukryj_recznie();
    assert!(!r.widoczna(), "pokazany mimo pełnego ekranu też się chowa");
}

#[test]
fn ukrycie_reczne_i_przez_pelny_ekran() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    assert!(r.widoczna());
    r.ustaw_pelny_ekran(true);
    assert!(!r.widoczna());
    r.przelacz_widocznosc();
    assert!(r.widoczna(), "skrót pokazuje widget mimo pełnego ekranu");
    r.ustaw_pelny_ekran(false);
    r.przelacz_widocznosc();
    assert!(!r.widoczna());
    r.ustaw_pelny_ekran(true);
    r.ustaw_pelny_ekran(false);
    assert!(!r.widoczna(), "ręcznie schowany zostaje schowany po pełnym ekranie");
    r.pokaz();
    assert!(r.widoczna());
}

#[test]
fn dzien_z_wersji_1_zachowuje_linie_w_dzienniku() {
    let sr = Srodowisko::nowe();
    fs::create_dir_all(sr.lokalny.join("dni")).unwrap();
    fs::write(
        sr.lokalny.join("dni").join("2026-10-30.json"),
        r##"{"data": "2026-10-30", "sesje": [{"id": "12", "nazwa": "Scenariusz: intro", "szac_min": 60,
              "start": "2026-10-30T09:12:00+01:00", "koniec": "2026-10-30T10:05:00+01:00",
              "sekundy": 2820, "minuty": 47, "status": "Zrobione"}]}"##,
    )
    .unwrap();
    let mut r = sr.rdzen(t("11:00:00"));
    assert_eq!(r.stan(), Stan::WyborZadan, "dzień z wersji 1 miał już powitanie");
    r.tick(t("11:00:06"));
    r.zatwierdz_wybor(t("11:00:10"));
    r.rozpocznij(&montaz(), t("11:10:00")).unwrap();
    r.przerwij(t("11:40:00"));
    let dziennik = sr.dziennik("2026-10-30");
    assert!(dziennik.contains("- [x] 09:12–10:05 · Scenariusz: intro · 47 min (szac. 60)\n"));
    assert!(dziennik.contains("- [ ] 11:10–11:40 · Montaż: plansze 1–5 · 30 min · w toku · Film #1 · FIFA\n"));
    assert!(dziennik.contains("minuty: 77\n"));
}

#[test]
fn polozenie_widgetu_trafia_do_okno_json_i_do_migawki() {
    use crate::okno::{MonitorOkna, Naroznik, Pion, Strona};
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("08:00:00"));
    let m = r.migawka();
    assert_eq!((m.strona, m.pion), (Strona::Prawa, Pion::Dol), "domyślnie prawy dolny róg");

    let drugi = MonitorOkna { nazwa: "\\\\.\\DISPLAY2".into(), x: 1920, y: 0, szer: 2560, wys: 1440 };
    let kotwica = KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::LewyGorny, odstep_x: 40.0, odstep_y: 20.0 };
    r.ustaw_kotwice(kotwica.clone(), t("08:01:00"));
    let m = r.migawka();
    assert_eq!(
        (m.strona, m.pion),
        (Strona::Lewa, Pion::Gora),
        "lewa połowa monitora: postać odbita, panel rośnie w dół"
    );
    assert_eq!(sr.nowy_rdzen(t("08:02:00")).kotwica(), kotwica, "po restarcie widget stoi tam, gdzie go zostawiono");
}

#[test]
fn zastepczy_rog_nie_nadpisuje_zapisanej_pozycji() {
    use crate::okno::{MonitorOkna, Naroznik};
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("08:00:00"));
    let drugi = MonitorOkna { nazwa: "DP-2".into(), x: 1600, y: 0, szer: 1600, wys: 1000 };
    let kotwica = KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::PrawyGorny, odstep_x: 10.0, odstep_y: 10.0 };
    r.ustaw_kotwice(kotwica.clone(), t("08:01:00"));
    // monitor zniknął: okno stoi w domyślnym rogu, ale zapis zostaje
    r.ustaw_naroznik(Naroznik::PrawyDolny);
    assert_eq!(r.migawka().pion, Pion::Dol);
    assert_eq!(sr.nowy_rdzen(t("08:02:00")).kotwica(), kotwica);
}

#[test]
fn przywroc_pozycje_wraca_na_monitor_glowny() {
    use crate::okno::{MonitorOkna, Naroznik};
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("08:00:00"));
    let drugi = MonitorOkna { nazwa: "DP-2".into(), x: 1600, y: 0, szer: 1600, wys: 1000 };
    r.ustaw_kotwice(
        KotwicaOkna { monitor: Some(drugi), kotwica: Naroznik::LewyGorny, odstep_x: 300.0, odstep_y: 200.0 },
        t("08:01:00"),
    );
    r.przywroc_pozycje(t("08:02:00"));
    assert_eq!(r.kotwica(), KotwicaOkna::default());
    assert_eq!(r.migawka().strona, Strona::Prawa);
    assert_eq!(sr.nowy_rdzen(t("08:03:00")).kotwica(), KotwicaOkna::default());
}

// ---- Zrobione od razu w pliku i Cofnij (2.1) -----------------------------------------------

fn zaczynamy_miniature(sr: &Srodowisko) -> Rdzen {
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));
    r.rozpocznij(&miniatura(), t("11:30:00")).unwrap();
    r.wez_zdarzenia();
    r
}

const MINIATURA_ZROBIONA: &str = "- [x] Miniatura [szac:: 30] [czas:: 40] ✅ 2026-10-30";

#[test]
fn zrobione_zapisuje_ptaszka_w_pliku_od_razu_bez_czekania_na_dymek_i_koniec_dnia() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.zrobione(t("12:10:00"));
    // Bez ticka, bez cyklu, bez zamknięcia dymku: plik jest już gotowy.
    assert!(sr.zadania().contains(MINIATURA_ZROBIONA), "{}", sr.zadania());
    assert!(r.dz.oczekujace.is_empty());
    assert!(sr.dziennik("2026-10-30").contains("- [x] 11:30–12:10 · Miniatura"));
}

#[test]
fn przed_kazdym_zapisem_pliku_zadan_zostaje_kopia_w_folderze_widgetu() {
    let sr = Srodowisko::nowe();
    let kopie = || {
        let mut n: Vec<PathBuf> =
            fs::read_dir(sr.lokalny.join("kopie")).map(|d| d.map(|w| w.unwrap().path()).collect()).unwrap_or_default();
        n.sort();
        n
    };
    let mut r = zaczynamy_miniature(&sr);
    assert!(kopie().is_empty(), "samo wybranie zadania niczego nie zapisuje");
    r.zrobione(t("12:10:00"));
    assert_eq!(kopie().len(), 1);
    assert_eq!(fs::read_to_string(&kopie()[0]).unwrap(), ZADANIA, "kopia to plik sprzed „Zrobione”");
    // „Dodaj zadanie” też zmienia plik, więc też odkłada kopię: plik z ptaszkiem, bez nowego zadania.
    r.dodaj_zadanie("Nowe zadanie", Some(10), "Inbox").unwrap();
    assert_eq!(kopie().len(), 2);
    let druga = fs::read_to_string(&kopie()[1]).unwrap();
    assert!(druga.contains(MINIATURA_ZROBIONA) && !druga.contains("Nowe zadanie"), "{druga}");
    assert!(sr.zadania().contains("- [ ] Nowe zadanie [szac:: 10]"));
    assert!(fs::read_dir(&sr.vault).unwrap().all(|w| w.unwrap().file_name() != "kopie"), "w vaulcie nie ma kopii");
}

#[test]
fn zajety_plik_jest_ponawiany_po_sekundzie_a_nie_po_30_sekundach() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    let schowany = sr.plik().with_extension("zajety");
    fs::rename(sr.plik(), &schowany).unwrap();
    r.zrobione(t("12:10:00"));
    assert_eq!(r.dz.oczekujace.len(), 1, "zapis czeka, bo plik jest niedostępny");
    assert_eq!(r.najblizszy_termin(t("12:10:00")), t("12:10:01"), "ponowienie za sekundę");
    fs::rename(&schowany, sr.plik()).unwrap();
    r.tick(t("12:10:01"));
    assert!(r.dz.oczekujace.is_empty(), "po sekundzie, bez cyklu co 30 s");
    assert!(sr.zadania().contains(MINIATURA_ZROBIONA), "{}", sr.zadania());
}

#[test]
fn ponowienia_zapisu_zwalniaja_ale_nie_przekraczaja_10_sekund() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    fs::rename(sr.plik(), sr.plik().with_extension("zajety")).unwrap();
    r.zrobione(t("12:10:00"));
    let mut teraz = t("12:10:00");
    let mut odstepy = Vec::new();
    for _ in 0..8 {
        let nastepny = r.zapis_ponow.expect("zapis czeka na ponowienie");
        odstepy.push((nastepny - teraz).num_seconds());
        teraz = nastepny;
        r.tick(teraz);
    }
    assert_eq!(odstepy, [1, 1, 2, 3, 5, 10, 10, 10]);
}

#[test]
fn cofnij_zdejmuje_ptaszka_i_wraca_do_pracy_nad_tym_samym_zadaniem() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.zrobione(t("12:10:00"));
    assert_eq!(r.stan(), Stan::Gratulacje);
    assert!(r.migawka().gratulacje.unwrap().mozna_cofnac);
    r.wez_zdarzenia();

    r.cofnij(t("12:10:07"));
    assert_eq!(
        sr.zadania(),
        ZADANIA.replace("- [ ] Miniatura [szac:: 30]", "- [ ] Miniatura [szac:: 30] [czas:: 40]"),
        "ptaszek i data zdjęte, zapisany czas zostaje"
    );
    let dziennik = sr.dziennik("2026-10-30");
    assert!(dziennik.contains("- [ ] 11:30–12:10 · Miniatura · 40 min · w toku · Film #1 · FIFA\n"), "{dziennik}");
    assert!(dziennik.contains("zrobione: 0\n"), "{dziennik}");
    assert_eq!(r.stan(), Stan::Praca);
    let s = r.migawka().sesja.unwrap();
    assert_eq!(s.zadanie, "Miniatura");
    assert_eq!(s.sekundy_zamkniete, 40 * 60, "bez utraty czasu: licznik zaczyna od zapisanych 40 min");
    assert_eq!(wstawki(&r.wez_zdarzenia(), "cofniete")["zadanie"], "Miniatura");
    assert!(r.migawka().gratulacje.is_none());
}

#[test]
fn po_cofnieciu_kolejne_zrobione_znowu_zapisuje_ptaszka_i_czas() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.zrobione(t("12:10:00"));
    r.cofnij(t("12:10:05"));
    r.zrobione(t("12:25:05"));
    assert!(sr.zadania().contains("- [x] Miniatura [szac:: 30] [czas:: 55] ✅ 2026-10-30"), "{}", sr.zadania());
    let dziennik = sr.dziennik("2026-10-30");
    assert!(dziennik.contains("- [ ] 11:30–12:10 · Miniatura · 40 min · w toku"), "{dziennik}");
    assert!(dziennik.contains("- [x] 12:10–12:25 · Miniatura · 15 min (szac. 30)"), "{dziennik}");
}

#[test]
fn cofnij_przy_duplikacie_nad_starym_zadaniem_przywraca_plik_sprzed_zrobione() {
    let sr = Srodowisko::nowe();
    let przed = "## P\n- [ ] Przegląd maili [czas:: 10]\n- [x] Przegląd maili [czas:: 10] ✅ 2026-10-01\n";
    fs::write(sr.plik(), przed).unwrap();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    r.rozpocznij(&k("P", "Przegląd maili"), t("10:00:00")).unwrap();
    r.zrobione(t("10:00:10"));
    assert!(sr.zadania().contains("✅ 2026-10-30"));
    r.cofnij(t("10:00:15"));
    assert_eq!(sr.zadania(), przed);
}

#[test]
fn cofnij_przed_zapisem_ptaszka_w_zajetym_pliku_nie_dopisuje_go_wcale() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    let schowany = sr.plik().with_extension("zajety");
    fs::rename(sr.plik(), &schowany).unwrap();
    r.zrobione(t("12:10:00"));
    r.cofnij(t("12:10:03"));
    assert_eq!(r.stan(), Stan::Praca, "wracamy do pracy mimo zajętego pliku");
    fs::rename(&schowany, sr.plik()).unwrap();
    r.tick(t("12:10:10"));
    assert_eq!(
        sr.zadania(),
        ZADANIA.replace("- [ ] Miniatura [szac:: 30]", "- [ ] Miniatura [szac:: 30] [czas:: 40]"),
        "czas zapisany, ptaszka nie ma"
    );
    assert!(r.dz.oczekujace.is_empty());
}

#[test]
fn cofnij_po_10_sekundach_albo_poza_gratulacjami_nic_nie_robi() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.cofnij(t("11:31:00"));
    assert_eq!(r.stan(), Stan::Praca, "w Pracy nie ma czego cofać");
    r.zrobione(t("12:10:00"));
    r.tick(t("12:10:10"));
    assert_eq!(r.stan(), Stan::Lista);
    r.cofnij(t("12:10:11"));
    assert_eq!(r.stan(), Stan::Lista);
    assert!(sr.zadania().contains(MINIATURA_ZROBIONA));
}

#[test]
fn gratulacje_trwaja_10_sekund_a_klikniecie_w_dymek_idzie_do_listy_od_razu() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.zrobione(t("12:10:00"));
    r.tick(t("12:10:09"));
    assert_eq!(r.stan(), Stan::Gratulacje, "przez 9 s dymek z przyciskiem Cofnij jeszcze trwa");
    r.dalej_po_gratulacjach();
    assert_eq!(r.stan(), Stan::Lista);
    assert!(dymki(&mut r).contains(&"zostalo".to_string()));
}

#[test]
fn po_ostatnim_zadaniu_cofnij_dziala_przez_10_sekund_potem_zostaja_przyciski() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.zatwierdz_wybor(t("09:00:10"));
    for (k, od, do_) in [(montaz(), "09:10:00", "09:40:00"), (miniatura(), "09:41:00", "10:11:00")] {
        r.rozpocznij(&k, t(od)).unwrap();
        r.zrobione(t(do_));
        r.tick(t(&format!("{}0", &do_[..7])));
    }
    let g = r.migawka().gratulacje.unwrap();
    assert!(g.plan_wykonany);
    r.tick(t("10:11:10"));
    assert!(!r.migawka().gratulacje.unwrap().mozna_cofnac, "po 10 s Cofnij znika, zostają przyciski końca");
    r.cofnij(t("10:11:11"));
    assert_eq!(r.stan(), Stan::Gratulacje);
}

// ---- zajęty skrót (2.1) -------------------------------------------------------------------

#[test]
fn zajety_skrot_daje_jeden_dymek_po_powitaniu_i_nastepnym_dymku() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    assert_eq!(r.stan(), Stan::Powitanie);
    r.skrot_zajety("Ctrl+Alt+Shift+Space", t("09:00:13"));
    r.wez_zdarzenia();
    r.tick(t("09:00:06"));
    r.tick(t("09:00:12"));
    assert!(!dymki(&mut r).contains(&"skrot_zajety".to_string()), "nie zagłusza powitania ani „Co bierzemy na dziś?”");
    assert_eq!(r.najblizszy_termin(t("09:00:12")), t("09:00:13"));
    r.tick(t("09:00:13"));
    let zd = r.wez_zdarzenia();
    assert_eq!(wstawki(&zd, "skrot_zajety")["skrot"], "Ctrl+Alt+Shift+Spacja");
    r.tick(t("09:00:20"));
    assert!(dymki(&mut r).is_empty(), "dymek tylko raz");
    assert!(r.najblizszy_termin(t("09:00:20")) > t("09:00:30"), "nic już nie budzi wątku");
}

#[test]
fn zajety_skrot_czeka_na_koniec_powitania_i_kreatora() {
    let sr = Srodowisko::nowe();
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
    r.wymus_kreator();
    r.start(t("09:00:00"));
    r.skrot_zajety("Ctrl+Alt+Shift+Space", t("09:00:13"));
    r.wez_zdarzenia();
    r.tick(t("09:05:00"));
    assert!(dymki(&mut r).is_empty(), "w kreatorze dymek o skrócie nie przeszkadza");
    r.zakoncz_kreator(sr.ustawienia(), t("09:06:00"));
    assert_eq!(dymki(&mut r), vec!["konfiguracja_gotowe"]);
    r.tick(t("09:06:05"));
    assert!(dymki(&mut r).is_empty(), "najpierw „Gotowe!”");
    r.tick(t("09:06:08"));
    assert_eq!(dymki(&mut r), vec!["skrot_zajety"]);
}

#[test]
fn zmiana_skrotu_w_ustawieniach_mowi_o_konflikcie_od_razu() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    r.tick(t("09:00:06"));
    r.tick(t("09:00:12"));
    r.wez_zdarzenia();
    r.skrot_zajety("Ctrl+Shift+F9", t("11:00:00"));
    r.tick(t("11:00:00"));
    assert_eq!(wstawki(&r.wez_zdarzenia(), "skrot_zajety")["skrot"], "Ctrl+Shift+F9");
}

// ---- sekwencja przerwy (2.1) ------------------------------------------------------------

#[test]
fn piec_sekund_po_pauzie_postac_mowi_ze_gra_ale_tylko_raz() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.pauza(t("11:40:00"));
    assert_eq!(dymki(&mut r), vec!["pauza"]);
    assert_eq!(r.najblizszy_termin(t("11:40:00")), t("11:40:05"));
    r.tick(t("11:40:04"));
    assert!(dymki(&mut r).is_empty(), "przez pierwsze 5 s postać się przeciąga");
    r.tick(t("11:40:05"));
    assert_eq!(dymki(&mut r), vec!["pauza_gra"]);
    r.tick(t("11:40:30"));
    assert!(dymki(&mut r).is_empty(), "dymek tylko raz");
    assert!(r.najblizszy_termin(t("11:40:30")) > t("11:41:30"), "nic już nie budzi wątku");
}

#[test]
fn kolejna_przerwa_znowu_dostaje_dymek_o_graniu() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.pauza(t("11:40:00"));
    r.tick(t("11:40:05"));
    r.wez_zdarzenia();
    r.wznow(t("11:45:00"));
    r.pauza(t("11:50:00"));
    r.wez_zdarzenia();
    r.tick(t("11:50:05"));
    assert_eq!(dymki(&mut r), vec!["pauza_gra"]);
}

#[test]
fn przerwa_z_blokady_ekranu_i_spozniony_tick_nie_daja_dymku_o_graniu() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.pauza_automatyczna(PowodPauzy::Blokada, t("11:40:00"));
    r.wez_zdarzenia();
    r.tick(t("11:40:05"));
    r.tick(t("11:55:00"));
    assert!(dymki(&mut r).is_empty(), "blokada ekranu: nikt nie patrzy");
    // ręczna przerwa, a wątek obudził się dopiero po minucie (np. po uśpieniu)
    r.wznow(t("12:00:00"));
    r.pauza(t("12:10:00"));
    r.wez_zdarzenia();
    r.tick(t("12:11:00"));
    assert!(dymki(&mut r).is_empty(), "spóźniony dymek byłby bez sensu");
    assert!(r.najblizszy_termin(t("12:11:00")) > t("12:12:00"));
}

// ---- imię postaci (2.1) -----------------------------------------------------------------

#[test]
fn kazdy_dymek_niesie_imie_postaci_do_wstawki_postac() {
    let sr = Srodowisko::nowe();
    let ust = Ustawienia { nazwa_postaci: "Bajtka".into(), ..sr.ustawienia() };
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), ust, t("09:00:00"));
    r.start(t("09:00:00"));
    let zd = r.wez_zdarzenia();
    assert_eq!(wstawki(&zd, "powitanie")["postac"], "Bajtka");
    assert_eq!(wstawki(&zd, "powitanie")["imie"], "Dżiordżio", "forma zwracania się to osobne pole");
}

#[test]
fn kreator_przedstawia_sie_imieniem_postaci_a_zmiana_w_ustawieniach_dziala_od_razu() {
    let sr = Srodowisko::nowe();
    let mut r = Rdzen::nowy(Magazyn::new(sr.lokalny.clone()), Ustawienia::default(), t("09:00:00"));
    r.wymus_kreator();
    r.start(t("09:00:00"));
    assert_eq!(wstawki(&r.wez_zdarzenia(), "konfiguracja_imie")["postac"], "Sai");
    r.zakoncz_kreator(sr.ustawienia(), t("09:01:00"));
    r.wez_zdarzenia();
    r.zmien_ustawienia(Ustawienia { nazwa_postaci: "才ちゃん".into(), ..sr.ustawienia() }, t("09:02:00"));
    r.skrot_zajety("Ctrl+Shift+F9", t("09:02:00"));
    r.tick(t("09:02:10"));
    assert_eq!(wstawki(&r.wez_zdarzenia(), "skrot_zajety")["postac"], "才ちゃん");
}

// ---- dźwięk skupienia (2.1) -------------------------------------------------------------

#[test]
fn dzwiek_gra_tylko_gdy_licznik_liczy_czas_zadania() {
    let sr = Srodowisko::nowe();
    let mut r = sr.rdzen(t("09:00:00"));
    assert_eq!(r.dzwiek_cel(), None, "powitanie");
    r.tick(t("09:00:06"));
    assert_eq!(r.dzwiek_cel(), None, "wybór zadań");
    r.przelacz_wybor(&miniatura());
    r.zatwierdz_wybor(t("09:00:30"));
    assert_eq!(r.dzwiek_cel(), None, "lista");
    r.rozpocznij(&miniatura(), t("11:30:00")).unwrap();
    assert_eq!(r.stan(), Stan::Praca);
    assert_eq!(r.dzwiek_cel(), Some(0.25), "praca: domyślna głośność 25");
    r.pauza(t("11:40:00"));
    assert_eq!(r.dzwiek_cel(), None, "pauza");
    r.wznow(t("11:45:00"));
    assert_eq!(r.dzwiek_cel(), Some(0.25), "wznowienie");
    r.bezczynnosc(15 * 60, t("12:00:00"));
    assert_eq!(r.stan(), Stan::Jestes);
    assert_eq!(r.dzwiek_cel(), None, "„Jesteś?”");
    r.odpowiedz_jestes(false, t("12:01:00"));
    assert_eq!(r.dzwiek_cel(), Some(0.25));
    r.pauza_automatyczna(PowodPauzy::Blokada, t("12:05:00"));
    assert_eq!(r.dzwiek_cel(), None, "blokada ekranu");
    r.powrot(t("12:20:00"));
    r.wznow(t("12:20:05"));
    assert_eq!(r.dzwiek_cel(), Some(0.25));
    r.pauza_automatyczna(PowodPauzy::Uspienie, t("12:30:00"));
    assert_eq!(r.dzwiek_cel(), None, "uśpienie");
    r.wznow(t("12:40:00"));
    r.zrobione(t("12:50:00"));
    assert_eq!(r.stan(), Stan::Gratulacje);
    assert_eq!(r.dzwiek_cel(), None, "Zrobione");
    r.tick(t("12:50:10"));
    r.rozpocznij(&montaz(), t("13:00:00")).unwrap();
    r.przerwij(t("13:30:00"));
    assert_eq!(r.dzwiek_cel(), None, "Przerwij");
    r.rozpocznij(&montaz(), t("13:35:00")).unwrap();
    r.koniec_pracy(t("13:50:00"));
    assert_eq!(r.dzwiek_cel(), None, "koniec dnia");
}

#[test]
fn dzwiek_wylaczony_w_ustawieniach_albo_wyciszony_przez_obs_nie_gra() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    assert_eq!(r.dzwiek_cel(), Some(0.25));
    let ust = r.ust.clone();
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia: false, ..ust.clone() }, t("11:31:00"));
    assert_eq!(r.dzwiek_cel(), None, "wyłączony przełącznikiem");
    assert!(!r.migawka().dzwiek.wlaczony);
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia_glosnosc: 60, ..ust.clone() }, t("11:32:00"));
    assert_eq!(r.dzwiek_cel(), Some(0.6));
    r.ustaw_obs(true);
    assert_eq!(r.dzwiek_cel(), None, "OBS i cisza przy nagrywaniu");
    let w = r.migawka().dzwiek;
    assert!(w.wlaczony && !w.gra && w.wyciszony_przez_obs, "etykieta jest przygaszona");
    r.zmien_ustawienia(Ustawienia { cisza_przy_nagrywaniu: false, ..ust }, t("11:33:00"));
    assert_eq!(r.dzwiek_cel(), Some(0.25), "bez „Cisza przy nagrywaniu” OBS nie przeszkadza");
    assert!(r.migawka().dzwiek.gra);
    r.ustaw_obs(false);
    r.ustaw_dzwiek_niedostepny(true);
    assert!(!r.migawka().dzwiek.gra, "brak urządzenia audio: etykieta przygaszona");
    assert!(!r.dzwiek_slychac());
}

#[test]
fn suwak_na_zero_to_cisza_bez_znacznika_i_bez_zielonej_etykiety() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    let ust = r.ust.clone();
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia_glosnosc: 0, ..ust }, t("11:31:00"));
    assert_eq!(r.dzwiek_cel(), None);
    assert!(!r.dzwiek_slychac());
    let w = r.migawka().dzwiek;
    assert!(w.wlaczony && !w.gra, "przełącznik zaznaczony, ale nic nie gra");
}

#[test]
fn sesja_z_dzwiekiem_przez_wiekszosc_czasu_ma_znacznik_40_hz_w_dzienniku() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.synchronizuj_dzwiek(t("11:30:00"));
    r.pauza(t("11:50:00"));
    r.synchronizuj_dzwiek(t("11:50:00"));
    r.wznow(t("11:55:00"));
    r.synchronizuj_dzwiek(t("11:55:00"));
    r.zrobione(t("12:15:00"));
    let dziennik = sr.dziennik("2026-10-30");
    assert!(
        dziennik.contains("- [x] 11:30–12:15 · Miniatura · 40 min (szac. 30) · Film #1 · FIFA · 40 Hz\n"),
        "{dziennik}"
    );
    assert!(r.dz.sesje.last().unwrap().dzwiek);
}

#[test]
fn sesja_bez_dzwieku_albo_z_dzwiekiem_przez_mniej_niz_polowe_nie_ma_znacznika() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    // dźwięk wyłączony w ustawieniach przez całą sesję
    let ust = r.ust.clone();
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia: false, ..ust.clone() }, t("11:30:00"));
    r.synchronizuj_dzwiek(t("11:30:00"));
    r.przerwij(t("12:10:00"));
    assert!(!sr.dziennik("2026-10-30").contains("40 Hz"));
    // druga sesja: dźwięk włączony dopiero na ostatnie 10 z 40 minut
    r.zmien_ustawienia(ust, t("12:15:00"));
    r.tick(t("12:15:10"));
    r.rozpocznij(&miniatura(), t("12:20:00")).unwrap();
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia: false, ..r.ust.clone() }, t("12:20:00"));
    r.synchronizuj_dzwiek(t("12:20:00"));
    r.zmien_ustawienia(Ustawienia { dzwiek_skupienia: true, ..r.ust.clone() }, t("12:50:00"));
    r.synchronizuj_dzwiek(t("12:50:00"));
    r.przerwij(t("13:00:00"));
    assert!(!sr.dziennik("2026-10-30").contains("40 Hz"), "10 z 40 minut to nie większość");
}

#[test]
fn licznik_dzwieku_przezywa_restart_i_obs_przerywa_odcinek_dzwieku() {
    let sr = Srodowisko::nowe();
    let mut r = zaczynamy_miniature(&sr);
    r.synchronizuj_dzwiek(t("11:30:00"));
    r.ustaw_obs(true); // OBS startuje po 10 minutach i wycisza dźwięk
    r.synchronizuj_dzwiek(t("11:40:00"));
    assert_eq!(r.dz.sesja.as_ref().unwrap().dzwiek_s, 600);
    r.ustaw_obs(false);
    r.synchronizuj_dzwiek(t("11:50:00"));
    r.zapisz_stan(t("11:55:00"));
    let r2 = sr.rdzen(t("12:30:00"));
    let s = r2.dz.sesja.as_ref().unwrap();
    assert_eq!(s.dzwiek_s, 600 + 5 * 60, "10 min przed OBS i 5 min po, do ostatniego zapisu");
    assert_eq!(s.dzwiek_od, None, "po awarii dźwięk nie gra");
}
