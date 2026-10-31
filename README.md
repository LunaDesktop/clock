# Luna Clock (`luna-clock`)

> [Polski](README.pl.md) · [English](README.en.md) · [Deutsch](README.de.md)

Zegar lokalny i światowy, minutnik, stoper oraz prywatny kalendarz środowiska
Luna na libLunie.

```sh
cargo run                       # bieżący dzień
cargo run -- --date 2026-12-24  # otwiera kalendarz na wybranym dniu
cargo test --lib
cargo run --bin preview         # zrzuty do ../../docs/images/clock-*.png
```

## Funkcje

- **Zegar** — duży czas lokalny (12/24 h, sekundy), data, strefa, najbliższe
  wydarzenie dnia, kilka zegarów światowych (strefy IANA z DST).
- **Minutnik** — dowolne godziny/minuty/sekundy, presety, start/pauza/reset,
  pasek postępu i powiadomienie o końcu.
- **Stoper** — start/pauza, okrążenia z międzyczasem, reset.
- **Kalendarz** — miesiąc z kropkami wydarzeń i świąt, dodawanie/edycja/usuwanie
  wydarzeń, wydarzenia całodniowe i minutowe, import/eksport `.ics`.
- **Automatyczny motyw** — ciemno/jasno od wybranych godzin; harmonogram czyta
  także powłoka Luny, więc działa po zamknięciu aplikacji.

## Prywatność

Wydarzenia są zapisywane wyłącznie w
`$XDG_DATA_HOME/luna/clock/events.tsv`. Święta są liczone **offline** z
wbudowanych reguł — aplikacja nie wykonuje żadnych zapytań sieciowych i nie
wysyła danych. Import `.ics` czyta tylko wskazany plik lokalny.

Panel współpracuje z aplikacją przez ten sam plik wydarzeń: pokazuje kropki i
plan wybranego dnia oraz przycisk **Open calendar**.

Pełny opis: [`docs/clock.md`](../../docs/clock.md).

## Licencja

GNU GPL v3.0 lub nowsza (`GPL-3.0-or-later`) — pełny tekst w [`LICENSE`](LICENSE).
© 2026 Luna Project.
