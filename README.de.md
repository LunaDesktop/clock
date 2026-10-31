# Luna Clock

Kalender und Termine für Luna – Monatsansicht, Termine, ICS-Import/-Export.

> **Sprachen:** [Polski](README.pl.md) · [English](README.en.md) · [Deutsch](README.de.md)

## Teil des Luna-Projekts

Dieses Repository ist Teil der Organisation [LunaDesktop](https://github.com/LunaDesktop) und des **Luna**-Projekts — ein Desktop in Rust, ohne GTK, Qt oder GObject.

## Bauen

```bash
git clone https://github.com/LunaDesktop/clock.git
cd clock
cargo build --release
```

Anforderungen: Linux, Rust 1.80+, Wayland (für Fensteranwendungen).

## Starten

```bash
cargo run --release
```

## Dokumentation

- Projektseite: <https://luna.paganlinux.eu/>
- Entwicklerdokumentation: <https://luna.paganlinux.eu/en/docs/>
- Polnische Dokumentation: <https://luna.paganlinux.eu/docs/>
- API-Index: <https://luna.paganlinux.eu/en/docs/api.html>

## Lizenz

Der Code steht unter der Lizenz **GPL-3.0-or-later**. Der vollständige Text liegt in [LICENSE](LICENSE).
