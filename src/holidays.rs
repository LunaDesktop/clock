//! Święta publiczne liczone **lokalnie** z wbudowanych reguł.
//!
//! Nie ma tu żadnego zapytania sieciowego: kraj wybieramy z konfiguracji
//! (albo wykrywamy z locale/strefy), a daty — w tym Wielkanoc i święta
//! ruchome — wyliczamy w pamięci. Dzięki temu kalendarz nie zdradza, gdzie
//! ani kiedy użytkownik przebywa.

use crate::time::CivilDate;

/// Kraj obsługiwany przez wbudowany zestaw świąt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Country {
    pub code: &'static str,
    /// Angielski klucz i18n (np. `Poland`).
    pub name: &'static str,
}

/// Lista krajów w kolejności prezentacji.
pub const COUNTRIES: &[Country] = &[
    Country {
        code: "PL",
        name: "Poland",
    },
    Country {
        code: "DE",
        name: "Germany",
    },
    Country {
        code: "AT",
        name: "Austria",
    },
    Country {
        code: "CH",
        name: "Switzerland",
    },
    Country {
        code: "FR",
        name: "France",
    },
    Country {
        code: "BE",
        name: "Belgium",
    },
    Country {
        code: "NL",
        name: "Netherlands",
    },
    Country {
        code: "GB",
        name: "United Kingdom",
    },
    Country {
        code: "IE",
        name: "Ireland",
    },
    Country {
        code: "ES",
        name: "Spain",
    },
    Country {
        code: "PT",
        name: "Portugal",
    },
    Country {
        code: "IT",
        name: "Italy",
    },
    Country {
        code: "CZ",
        name: "Czechia",
    },
    Country {
        code: "SK",
        name: "Slovakia",
    },
    Country {
        code: "HU",
        name: "Hungary",
    },
    Country {
        code: "SI",
        name: "Slovenia",
    },
    Country {
        code: "HR",
        name: "Croatia",
    },
    Country {
        code: "LT",
        name: "Lithuania",
    },
    Country {
        code: "LV",
        name: "Latvia",
    },
    Country {
        code: "EE",
        name: "Estonia",
    },
    Country {
        code: "SE",
        name: "Sweden",
    },
    Country {
        code: "NO",
        name: "Norway",
    },
    Country {
        code: "DK",
        name: "Denmark",
    },
    Country {
        code: "FI",
        name: "Finland",
    },
    Country {
        code: "IS",
        name: "Iceland",
    },
    Country {
        code: "RO",
        name: "Romania",
    },
    Country {
        code: "BG",
        name: "Bulgaria",
    },
    Country {
        code: "GR",
        name: "Greece",
    },
    Country {
        code: "UA",
        name: "Ukraine",
    },
    Country {
        code: "US",
        name: "United States",
    },
    Country {
        code: "CA",
        name: "Canada",
    },
    Country {
        code: "AU",
        name: "Australia",
    },
    Country {
        code: "NZ",
        name: "New Zealand",
    },
    Country {
        code: "JP",
        name: "Japan",
    },
    Country {
        code: "CN",
        name: "China",
    },
];

/// Pojedyncze święto.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holiday {
    pub date: CivilDate,
    /// Angielski klucz i18n.
    pub name: &'static str,
}

/// Nazwa kraju (klucz i18n) albo sam kod, gdy kraj nie jest obsługiwany.
pub fn country_name(code: &str) -> &'static str {
    COUNTRIES
        .iter()
        .find(|country| country.code.eq_ignore_ascii_case(code))
        .map(|country| country.name)
        .unwrap_or("Unknown")
}

pub fn is_supported(code: &str) -> bool {
    COUNTRIES
        .iter()
        .any(|country| country.code.eq_ignore_ascii_case(code))
}

/// Kraj z locale (`pl_PL.UTF-8`, `en_GB`, `ja_JP`…).
pub fn country_from_locale(locale: &str) -> Option<&'static str> {
    let normalized: String = locale
        .chars()
        .map(|ch| if ch == '-' || ch == '.' { '_' } else { ch })
        .collect();
    let mut parts = normalized.split('_');
    let language = parts.next()?.to_ascii_lowercase();
    let region = parts.next().unwrap_or_default().to_ascii_uppercase();
    let region = region.split('@').next().unwrap_or_default();

    // Region jest ważniejszy: `en_GB` to nie `en_US`, a `de_CH` nie `de_DE`.
    if is_supported(region) {
        return COUNTRIES
            .iter()
            .find(|country| country.code == region)
            .map(|country| country.code);
    }
    match language.as_str() {
        "pl" => Some("PL"),
        "de" => Some("DE"),
        "fr" => Some("FR"),
        "it" => Some("IT"),
        "es" => Some("ES"),
        "pt" => Some("PT"),
        "nl" => Some("NL"),
        "cs" => Some("CZ"),
        "sk" => Some("SK"),
        "hu" => Some("HU"),
        "sl" => Some("SI"),
        "hr" => Some("HR"),
        "lt" => Some("LT"),
        "lv" => Some("LV"),
        "et" => Some("EE"),
        "sv" => Some("SE"),
        "nb" | "nn" | "no" => Some("NO"),
        "da" => Some("DK"),
        "fi" => Some("FI"),
        "is" => Some("IS"),
        "ro" => Some("RO"),
        "bg" => Some("BG"),
        "el" => Some("GR"),
        "uk" => Some("UA"),
        "ja" => Some("JP"),
        "zh" => Some("CN"),
        "en" => Some("US"),
        _ => None,
    }
}

/// Kraj ze strefy czasowej (przybliżenie — dla stref granicznych użytkownik
/// może wybrać kraj ręcznie).
pub fn country_from_timezone(timezone: &str) -> Option<&'static str> {
    let exact = match timezone {
        "Europe/Warsaw" => Some("PL"),
        "Europe/Berlin" | "Europe/Busingen" => Some("DE"),
        "Europe/Vienna" => Some("AT"),
        "Europe/Zurich" => Some("CH"),
        "Europe/Paris" => Some("FR"),
        "Europe/Brussels" => Some("BE"),
        "Europe/Amsterdam" => Some("NL"),
        "Europe/London" => Some("GB"),
        "Europe/Dublin" => Some("IE"),
        "Europe/Madrid" | "Atlantic/Canary" => Some("ES"),
        "Europe/Lisbon" | "Atlantic/Madeira" | "Atlantic/Azores" => Some("PT"),
        "Europe/Rome" => Some("IT"),
        "Europe/Prague" => Some("CZ"),
        "Europe/Bratislava" => Some("SK"),
        "Europe/Budapest" => Some("HU"),
        "Europe/Ljubljana" => Some("SI"),
        "Europe/Zagreb" => Some("HR"),
        "Europe/Vilnius" => Some("LT"),
        "Europe/Riga" => Some("LV"),
        "Europe/Tallinn" => Some("EE"),
        "Europe/Stockholm" => Some("SE"),
        "Europe/Oslo" => Some("NO"),
        "Europe/Copenhagen" => Some("DK"),
        "Europe/Helsinki" => Some("FI"),
        "Atlantic/Reykjavik" => Some("IS"),
        "Europe/Bucharest" => Some("RO"),
        "Europe/Sofia" => Some("BG"),
        "Europe/Athens" => Some("GR"),
        "Europe/Kyiv" | "Europe/Kiev" => Some("UA"),
        "Asia/Tokyo" => Some("JP"),
        "Asia/Shanghai" | "Asia/Chongqing" => Some("CN"),
        _ => None,
    };
    if exact.is_some() {
        return exact;
    }
    if matches!(
        timezone,
        "America/New_York"
            | "America/Chicago"
            | "America/Denver"
            | "America/Los_Angeles"
            | "America/Phoenix"
            | "America/Anchorage"
            | "Pacific/Honolulu"
            | "America/Detroit"
            | "America/Indiana/Indianapolis"
    ) {
        return Some("US");
    }
    if matches!(
        timezone,
        "America/Toronto"
            | "America/Vancouver"
            | "America/Edmonton"
            | "America/Winnipeg"
            | "America/Halifax"
            | "America/St_Johns"
            | "America/Regina"
            | "America/Whitehorse"
            | "America/Yellowknife"
            | "America/Iqaluit"
            | "America/Moncton"
            | "America/Glace_Bay"
            | "America/Goose_Bay"
    ) {
        return Some("CA");
    }
    if timezone.starts_with("Australia/") {
        return Some("AU");
    }
    if timezone.starts_with("Pacific/Auckland") || timezone.starts_with("Pacific/Chatham") {
        return Some("NZ");
    }
    if timezone.starts_with("Europe/") {
        return None;
    }
    None
}

/// Wykryty kraj: najpierw strefa czasowa („gdzie jesteś”), potem locale.
pub fn detect_country() -> Option<&'static str> {
    if let Some(code) = country_from_timezone(&libluna_core::region::current_timezone()) {
        return Some(code);
    }
    for key in ["LC_ALL", "LC_TIME", "LANG"] {
        if let Ok(value) = std::env::var(key) {
            if let Some(code) = country_from_locale(&value) {
                return Some(code);
            }
        }
    }
    None
}

/// Święta kraju w danym roku (posortowane po dacie).
pub fn year_holidays(country: &str, year: i32) -> Vec<Holiday> {
    let code = country_code_from_input(country);
    let mut holidays = Vec::new();
    let easter = easter_sunday(year);
    let fixed = |list: &[(u32, u32, &'static str)]| {
        list.iter()
            .map(|&(month, day, name)| Holiday {
                date: CivilDate::from_ymd(year, month, day.clamp(1, 31)),
                name,
            })
            .collect::<Vec<_>>()
    };
    let moving = |offsets: &[(i32, &'static str)]| {
        offsets
            .iter()
            .map(|&(offset, name)| Holiday {
                date: easter.add_days(offset as i64),
                name,
            })
            .collect::<Vec<_>>()
    };

    match code {
        "PL" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (5, 3, "Constitution Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (11, 11, "Independence Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "DE" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (10, 3, "German Unity Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "AT" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (8, 15, "Assumption of Mary"),
            (10, 26, "National Day"),
            (11, 1, "All Saints' Day"),
            (12, 8, "Immaculate Conception"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "CH" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 2, "Berchtold's Day"),
            (8, 1, "National Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "FR" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (5, 8, "Victory Day"),
            (7, 14, "Bastille Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (11, 11, "Armistice Day"),
            (12, 25, "Christmas Day"),
        ])),
        "BE" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (7, 21, "National Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (11, 11, "Armistice Day"),
            (12, 25, "Christmas Day"),
        ])),
        "NL" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (4, 27, "King's Day"),
            (5, 5, "Liberation Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "GB" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (12, 25, "Christmas Day"),
                (12, 26, "Boxing Day"),
            ]));
            holidays.push(Holiday {
                date: nth_weekday(year, 5, 0, 1),
                name: "Early May Bank Holiday",
            });
            holidays.push(Holiday {
                date: last_weekday(year, 5, 0),
                name: "Spring Bank Holiday",
            });
            holidays.push(Holiday {
                date: last_weekday(year, 8, 0),
                name: "Summer Bank Holiday",
            });
        }
        "IE" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (3, 17, "Saint Patrick's Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Saint Stephen's Day"),
        ])),
        "ES" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (8, 15, "Assumption of Mary"),
            (10, 12, "National Day"),
            (11, 1, "All Saints' Day"),
            (12, 6, "Constitution Day"),
            (12, 8, "Immaculate Conception"),
            (12, 25, "Christmas Day"),
        ])),
        "PT" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (4, 25, "Freedom Day"),
            (5, 1, "Labour Day"),
            (6, 10, "Portugal Day"),
            (8, 15, "Assumption of Mary"),
            (10, 5, "Republic Day"),
            (11, 1, "All Saints' Day"),
            (12, 1, "Restoration of Independence"),
            (12, 8, "Immaculate Conception"),
            (12, 25, "Christmas Day"),
        ])),
        "IT" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (4, 25, "Liberation Day"),
            (5, 1, "Labour Day"),
            (6, 2, "Republic Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (12, 8, "Immaculate Conception"),
            (12, 25, "Christmas Day"),
            (12, 26, "Saint Stephen's Day"),
        ])),
        "CZ" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (5, 8, "Victory Day"),
            (7, 5, "Saints Cyril and Methodius"),
            (7, 6, "Jan Hus Day"),
            (9, 28, "Statehood Day"),
            (10, 28, "Independent Czechoslovak State Day"),
            (11, 17, "Struggle for Freedom Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "SK" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (5, 8, "Victory Day"),
            (7, 5, "Saints Cyril and Methodius"),
            (8, 29, "National Uprising Day"),
            (9, 1, "Constitution Day"),
            (9, 15, "Our Lady of Sorrows"),
            (11, 1, "All Saints' Day"),
            (11, 17, "Struggle for Freedom Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "HU" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (3, 15, "Revolution Day"),
            (5, 1, "Labour Day"),
            (8, 20, "Saint Stephen's Day"),
            (10, 23, "Republic Day"),
            (11, 1, "All Saints' Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "SI" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 2, "New Year's Holiday"),
            (2, 8, "Prešeren Day"),
            (4, 27, "Uprising Day"),
            (5, 1, "Labour Day"),
            (5, 2, "Labour Day Holiday"),
            (6, 25, "Statehood Day"),
            (8, 15, "Assumption of Mary"),
            (10, 31, "Reformation Day"),
            (11, 1, "All Saints' Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Independence and Unity Day"),
        ])),
        "HR" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (5, 30, "Statehood Day"),
            (6, 22, "Anti-Fascist Struggle Day"),
            (8, 5, "Victory Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (11, 18, "Remembrance Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Saint Stephen's Day"),
        ])),
        "LT" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (2, 16, "Restoration of the State Day"),
            (3, 11, "Restoration of Independence Day"),
            (5, 1, "Labour Day"),
            (6, 24, "Saint John's Day"),
            (7, 6, "Statehood Day"),
            (8, 15, "Assumption of Mary"),
            (11, 1, "All Saints' Day"),
            (11, 2, "All Souls' Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "LV" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (5, 4, "Restoration of Independence Day"),
            (6, 23, "Midsummer Eve"),
            (6, 24, "Midsummer Day"),
            (11, 18, "Proclamation Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
            (12, 31, "New Year's Eve"),
        ])),
        "EE" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (2, 24, "Independence Day"),
            (5, 1, "Spring Day"),
            (6, 23, "Victory Day"),
            (6, 24, "Midsummer Day"),
            (8, 20, "Restoration of Independence Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "SE" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "Labour Day"),
            (6, 6, "National Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "NO" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (5, 17, "Constitution Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "DK" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "FI" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (5, 1, "May Day"),
            (12, 6, "Independence Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "IS" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (6, 17, "National Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "RO" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 2, "New Year's Holiday"),
            (1, 6, "Epiphany"),
            (1, 7, "Saint John the Baptist"),
            (1, 24, "Union Day"),
            (5, 1, "Labour Day"),
            (8, 15, "Assumption of Mary"),
            (11, 30, "Saint Andrew's Day"),
            (12, 1, "National Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "BG" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (3, 3, "Liberation Day"),
            (5, 1, "Labour Day"),
            (5, 6, "Saint George's Day"),
            (5, 24, "Bulgarian Education Day"),
            (9, 6, "Unification Day"),
            (9, 22, "Independence Day"),
            (12, 24, "Christmas Eve"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "GR" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 6, "Epiphany"),
            (3, 25, "Independence Day"),
            (5, 1, "Labour Day"),
            (8, 15, "Assumption of Mary"),
            (10, 28, "Ochi Day"),
            (12, 25, "Christmas Day"),
            (12, 26, "Second Day of Christmas"),
        ])),
        "UA" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (1, 7, "Orthodox Christmas Day"),
            (3, 8, "International Women's Day"),
            (5, 1, "Labour Day"),
            (5, 9, "Victory Day"),
            (6, 28, "Constitution Day"),
            (8, 24, "Independence Day"),
            (10, 14, "Defender of Ukraine Day"),
            (12, 25, "Christmas Day"),
        ])),
        "US" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (6, 19, "Juneteenth"),
                (7, 4, "Independence Day"),
                (11, 11, "Veterans Day"),
                (12, 25, "Christmas Day"),
            ]));
            holidays.push(Holiday {
                date: nth_weekday(year, 1, 0, 3),
                name: "Martin Luther King Jr. Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 2, 0, 3),
                name: "Presidents' Day",
            });
            holidays.push(Holiday {
                date: last_weekday(year, 5, 0),
                name: "Memorial Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 9, 0, 1),
                name: "Labor Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 10, 0, 2),
                name: "Columbus Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 11, 3, 4),
                name: "Thanksgiving Day",
            });
        }
        "CA" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (7, 1, "Canada Day"),
                (11, 11, "Remembrance Day"),
                (12, 25, "Christmas Day"),
                (12, 26, "Boxing Day"),
            ]));
            holidays.push(Holiday {
                date: last_weekday_before(year, 5, 24, 0),
                name: "Victoria Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 9, 0, 1),
                name: "Labour Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 10, 0, 2),
                name: "Thanksgiving Day",
            });
        }
        "AU" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (1, 26, "Australia Day"),
                (4, 25, "Anzac Day"),
                (12, 25, "Christmas Day"),
                (12, 26, "Boxing Day"),
            ]));
            holidays.push(Holiday {
                date: nth_weekday(year, 6, 0, 2),
                name: "King's Birthday",
            });
        }
        "NZ" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (1, 2, "Day after New Year's Day"),
                (2, 6, "Waitangi Day"),
                (4, 25, "Anzac Day"),
                (12, 25, "Christmas Day"),
                (12, 26, "Boxing Day"),
            ]));
            holidays.push(Holiday {
                date: nth_weekday(year, 6, 0, 1),
                name: "King's Birthday",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 10, 0, 4),
                name: "Labour Day",
            });
        }
        "JP" => {
            holidays.extend(fixed(&[
                (1, 1, "New Year's Day"),
                (2, 11, "National Foundation Day"),
                (2, 23, "Emperor's Birthday"),
                (4, 29, "Showa Day"),
                (5, 3, "Constitution Memorial Day"),
                (5, 4, "Greenery Day"),
                (5, 5, "Children's Day"),
                (8, 11, "Mountain Day"),
                (11, 3, "Culture Day"),
                (11, 23, "Labour Thanksgiving Day"),
            ]));
            holidays.push(Holiday {
                date: nth_weekday(year, 1, 0, 2),
                name: "Coming of Age Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 7, 0, 3),
                name: "Marine Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 9, 0, 3),
                name: "Respect for the Aged Day",
            });
            holidays.push(Holiday {
                date: nth_weekday(year, 10, 0, 2),
                name: "Sports Day",
            });
            holidays.push(Holiday {
                date: CivilDate::from_ymd(year, 3, vernal_equinox_day(year)),
                name: "Vernal Equinox Day",
            });
            holidays.push(Holiday {
                date: CivilDate::from_ymd(year, 9, autumn_equinox_day(year)),
                name: "Autumnal Equinox Day",
            });
        }
        "CN" => holidays.extend(fixed(&[
            (1, 1, "New Year's Day"),
            (5, 1, "Labour Day"),
            (10, 1, "National Day"),
        ])),
        _ => {}
    }

    // Ruchome święta chrześcijańskie zależne od Wielkanoci.
    let offset = match code {
        "PL" => Some(vec![
            (0, "Easter Sunday"),
            (1, "Easter Monday"),
            (49, "Pentecost Sunday"),
            (60, "Corpus Christi"),
        ]),
        "DE" | "AT" | "CH" | "SI" | "HR" | "HU" | "SK" | "CZ" => Some(vec![
            (0, "Easter Sunday"),
            (1, "Easter Monday"),
            (49, "Pentecost Sunday"),
            (50, "Whit Monday"),
        ]),
        "FR" | "BE" | "NL" | "IE" | "GB" => Some(vec![
            (0, "Easter Sunday"),
            (1, "Easter Monday"),
            (49, "Pentecost Sunday"),
            (50, "Whit Monday"),
        ]),
        "ES" | "PT" | "IT" => Some(vec![(0, "Easter Sunday"), (1, "Easter Monday")]),
        "SE" | "NO" | "DK" | "FI" | "IS" => Some(vec![
            (0, "Easter Sunday"),
            (1, "Easter Monday"),
            (49, "Pentecost Sunday"),
            (50, "Whit Monday"),
        ]),
        "RO" | "BG" | "GR" | "UA" => Some(vec![(0, "Easter Sunday"), (1, "Easter Monday")]),
        "LT" | "LV" | "EE" => Some(vec![(0, "Easter Sunday"), (1, "Easter Monday")]),
        _ => None,
    };
    if let Some(offset) = offset {
        holidays.extend(moving(&offset));
    }
    match code {
        "DE" | "AT" | "CH" => holidays.extend(moving(&[(-2, "Good Friday"), (39, "Ascension")])),
        "NL" | "BE" => {
            holidays.extend(moving(&[(-2, "Good Friday"), (39, "Ascension")]));
        }
        "GB" | "IE" => holidays.extend(moving(&[(-2, "Good Friday")])),
        "ES" | "PT" => holidays.extend(moving(&[(-2, "Good Friday")])),
        "SE" => holidays.extend(moving(&[(-2, "Good Friday"), (39, "Ascension")])),
        "NO" => holidays.extend(moving(&[
            (-3, "Maundy Thursday"),
            (-2, "Good Friday"),
            (39, "Ascension"),
        ])),
        "DK" | "FI" | "IS" => holidays.extend(moving(&[
            (-3, "Maundy Thursday"),
            (-2, "Good Friday"),
            (39, "Ascension"),
        ])),
        "LV" | "EE" => holidays.extend(moving(&[(-2, "Good Friday")])),
        "RO" | "BG" | "GR" => holidays.extend(moving(&[(-2, "Good Friday")])),
        _ => {}
    }
    if code == "PT" {
        holidays.extend(moving(&[(60, "Corpus Christi")]));
    }
    if matches!(code, "SE" | "FI") {
        holidays.push(Holiday {
            date: saturday_between(year, 6, 20, 26),
            name: "Midsummer Day",
        });
        holidays.push(Holiday {
            date: saturday_between(year, 10, 31, 6),
            name: "All Saints' Day",
        });
    }
    if code == "DK" {
        holidays.push(Holiday {
            date: easter.add_days(-3),
            name: "Maundy Thursday",
        });
    }
    if code == "PL" {
        holidays.retain(|holiday| holiday.name != "Good Friday");
    }

    holidays.sort_by_key(|holiday| holiday.date);
    holidays.dedup_by(|a, b| a.date == b.date && a.name == b.name);
    holidays
}

fn country_code_from_input(input: &str) -> &'static str {
    COUNTRIES
        .iter()
        .find(|country| country.code.eq_ignore_ascii_case(input))
        .map(|country| country.code)
        .unwrap_or("")
}

/// Święta kraju w danym dniu (np. do panelu szczegółów dnia).
pub fn holidays_on(country: &str, date: CivilDate) -> Vec<&'static str> {
    year_holidays(country, date.year)
        .into_iter()
        .filter(|holiday| holiday.date == date)
        .map(|holiday| holiday.name)
        .collect()
}

/// Święta kraju w danym miesiącu.
pub fn holidays_in_month(country: &str, year: i32, month: u32) -> Vec<Holiday> {
    year_holidays(country, year)
        .into_iter()
        .filter(|holiday| holiday.date.month == month)
        .collect()
}

/// Wielkanoc (algorytm Meeusa/Jonesa/Butchera, kalendarz gregoriański).
pub fn easter_sunday(year: i32) -> CivilDate {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31;
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    CivilDate::from_ymd(year, month as u32, day as u32)
}

fn nth_weekday(year: i32, month: u32, weekday: u32, n: u32) -> CivilDate {
    let first = CivilDate::from_ymd(year, month, 1);
    let delta = (weekday as i64 - first.weekday() as i64).rem_euclid(7);
    first.add_days(delta + (n.saturating_sub(1) as i64) * 7)
}

fn last_weekday(year: i32, month: u32, weekday: u32) -> CivilDate {
    let last = CivilDate::from_ymd(year, month, CivilDate::days_in_month(year, month));
    let delta = (last.weekday() as i64 - weekday as i64).rem_euclid(7);
    last.add_days(-delta)
}

fn last_weekday_before(year: i32, month: u32, day: u32, weekday: u32) -> CivilDate {
    let date = CivilDate::from_ymd(year, month, day);
    let delta = (date.weekday() as i64 - weekday as i64).rem_euclid(7);
    date.add_days(-delta)
}

fn saturday_between(year: i32, month: u32, from: u32, to: u32) -> CivilDate {
    if from <= to {
        let start = CivilDate::from_ymd(year, month, from);
        let delta = (5i64 - start.weekday() as i64).rem_euclid(7);
        start.add_days(delta)
    } else {
        // Zakres przechodzi przez koniec miesiąca (np. 31 października – 6 listopada).
        let start = CivilDate::from_ymd(year, month, from);
        let delta = (5i64 - start.weekday() as i64).rem_euclid(7);
        start.add_days(delta)
    }
}

fn vernal_equinox_day(year: i32) -> u32 {
    let days = 20.8431 + 0.242_194 * (year - 1980) as f64 - ((year - 1980) as f64 / 4.0).floor();
    days.floor().clamp(19.0, 21.0) as u32
}

fn autumn_equinox_day(year: i32) -> u32 {
    let days = 23.2488 + 0.242_194 * (year - 1980) as f64 - ((year - 1980) as f64 / 4.0).floor();
    days.floor().clamp(22.0, 24.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(country: &str, year: i32, month: u32, day: u32) -> Vec<&'static str> {
        holidays_on(country, CivilDate::from_ymd(year, month, day))
    }

    #[test]
    fn easter_dates_match_known_years() {
        assert_eq!(easter_sunday(2024), CivilDate::from_ymd(2024, 3, 31));
        assert_eq!(easter_sunday(2025), CivilDate::from_ymd(2025, 4, 20));
        assert_eq!(easter_sunday(2026), CivilDate::from_ymd(2026, 4, 5));
    }

    #[test]
    fn poland_has_fixed_and_moving_holidays() {
        let year = 2026;
        assert_eq!(names("PL", year, 1, 1), vec!["New Year's Day"]);
        assert_eq!(names("PL", year, 5, 3), vec!["Constitution Day"]);
        assert_eq!(names("PL", year, 4, 6), vec!["Easter Monday"]);
        assert_eq!(names("PL", year, 6, 4), vec!["Corpus Christi"]);
        assert_eq!(names("PL", year, 12, 26), vec!["Second Day of Christmas"]);
        assert!(year_holidays("PL", year).len() >= 13);
    }

    #[test]
    fn detects_country_from_locale_and_timezone() {
        assert_eq!(country_from_locale("pl_PL.UTF-8"), Some("PL"));
        assert_eq!(country_from_locale("en_GB.UTF-8"), Some("GB"));
        assert_eq!(country_from_locale("de_CH"), Some("CH"));
        assert_eq!(country_from_locale("ja_JP"), Some("JP"));
        assert_eq!(country_from_timezone("Europe/Warsaw"), Some("PL"));
        assert_eq!(country_from_timezone("America/New_York"), Some("US"));
        assert_eq!(country_from_timezone("Australia/Sydney"), Some("AU"));
    }

    #[test]
    fn computed_holidays_are_sane_for_several_countries() {
        let year = 2026;
        assert_eq!(names("US", year, 11, 26), vec!["Thanksgiving Day"]);
        assert_eq!(names("GB", year, 5, 4), vec!["Early May Bank Holiday"]);
        assert_eq!(names("JP", year, 2, 23), vec!["Emperor's Birthday"]);
        assert_eq!(names("CA", year, 7, 1), vec!["Canada Day"]);
    }

    #[test]
    fn unknown_country_is_empty() {
        assert!(year_holidays("XX", 2026).is_empty());
        assert!(holidays_on("XX", CivilDate::from_ymd(2026, 1, 1)).is_empty());
    }

    #[test]
    fn month_name_and_weekday_keys_exist() {
        assert_eq!(crate::time::month_name_key(1), "January");
        assert_eq!(crate::time::weekday_name_key(0), "Monday");
        assert_eq!(crate::time::weekday_name_key(6), "Sunday");
    }
}
