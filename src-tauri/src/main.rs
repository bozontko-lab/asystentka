// Bez okna konsoli w wersji release na Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    asystentka_lib::run()
}
