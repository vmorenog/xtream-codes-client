//! Resolving a **Category**'s **Region**, and spotting **Dividers** (ADR-0008).
//!
//! Both are heuristics over text a **Provider** wrote for humans, so both are
//! wrong sometimes. That is why a Region can be overridden by hand.

/// Region codes are ISO 3166-1 alpha-2 where the Region really is a country.
/// The three that are not get spelled out, because they have no ISO code and
/// because `AR` already means Argentina here — a **Provider** using `AR -` for
/// Arabic would otherwise collide with it.
pub const ARABIC: &str = "ARAB";
pub const LATAM: &str = "LATAM";
pub const EX_YUGOSLAVIA: &str = "EXYU";
/// A language, like `ARABIC`. Providers group VOD this way rather than by
/// country, so it needs a code of its own.
pub const ENGLISH: &str = "ENG";

/// Categories whose name gives nothing away — `ADULTS`, `NBA`, `PEACOCK`.
/// A real bucket rather than a null, so every Category has exactly one Region
/// to be filtered and ordered by, and the Viewer can see what went unmatched.
pub const OTHER: &str = "OTHER";

/// Two-letter prefixes, as they appear before ` - ` in a Category name.
///
/// `UK` is here because Providers use it; the Region it maps to is `GB`, the
/// same one Channel names spell `|GB|`. Keeping both pointing at one code is
/// the whole reason this table exists.
const PREFIXES: &[(&str, &str)] = &[
    ("AL", "AL"),
    ("AR", ARABIC), // Arabic, not Argentina — see the Providers' own usage.
    ("CH", "CH"),
    ("DE", "DE"),
    ("ES", "ES"),
    ("FR", "FR"),
    ("GB", "GB"),
    ("IT", "IT"),
    ("NL", "NL"),
    ("NZ", "NZ"),
    ("PL", "PL"),
    ("PT", "PT"),
    ("RO", "RO"),
    ("TR", "TR"),
    ("UK", "GB"),
    ("US", "US"),
];

/// Longer prefixes and bare names, matched case-insensitively against either
/// the whole Category name or the part before ` - `.
///
/// Misspellings are in here on purpose: `COLUMBIA`, `HUNGARIA` and `PAKISTANI`
/// are what the Provider actually ships, and correcting their spelling is not
/// our job — recognising it is.
const NAMES: &[(&str, &str)] = &[
    ("ALBANIA", "AL"),
    ("ARABIC", ARABIC),
    ("ARGENTINA", "AR"),
    ("AUSTRIA", "AT"),
    ("BELGIUM", "BE"),
    ("BOLIVIA", "BO"),
    ("BOSNA / HERSEK", "BA"),
    ("BOSNA", "BA"),
    ("BRAZIL", "BR"),
    ("BRASIL", "BR"),
    ("BULGARIA", "BG"),
    ("BULGARIAN", "BG"),
    ("CANADA", "CA"),
    ("CHILE", "CL"),
    ("COLOMBIA", "CO"),
    ("COLUMBIA", "CO"), // the Provider's spelling
    ("COSTA RICA", "CR"),
    ("CROATIA", "HR"),
    ("CZECH", "CZ"),
    ("DENMARK", "DK"),
    ("ECUADOR", "EC"),
    ("ENGLISH", ENGLISH),
    ("ESPANA", "ES"),
    ("ESPAÑA", "ES"),
    ("EX-YU", EX_YUGOSLAVIA),
    ("FINLAND", "FI"),
    ("GERMANY", "DE"),
    ("GREECE", "GR"),
    ("HOLLAND", "NL"), // not a country, but it is what they call it
    ("NETHERLANDS", "NL"),
    ("HUNGARIA", "HU"), // the Provider's spelling
    ("HUNGARY", "HU"),
    ("HUNGARIAN", "HU"),
    ("INDIA", "IN"),
    ("IRELAND", "IE"),
    ("LAT", LATAM),
    ("LATINO", LATAM),
    ("MEXICO", "MX"),
    ("NORWAY", "NO"),
    ("PAKISTAN", "PK"),
    ("PAKISTANI", "PK"), // the Provider's spelling
    ("PERU", "PE"),
    ("POLAND", "PL"),
    ("POLSKA", "PL"),
    ("PORTUGAL", "PT"),
    ("REPUBLICA DOMINICANA", "DO"),
    ("SERBIA", "RS"),
    ("SOUTH AFRICA", "ZA"),
    ("SPAIN", "ES"),
    ("SWEDEN", "SE"),
    ("SWISS", "CH"),
    ("SWITZERLAND", "CH"),
    ("TURKEY", "TR"),
    ("URUGUAY", "UY"),
    ("VENEZUELA", "VE"),
];

/// Human labels. Anything missing falls back to the code itself, which is ugly
/// but never wrong.
const LABELS: &[(&str, &str)] = &[
    ("AL", "Albania"),
    (ARABIC, "Arabic"),
    ("AR", "Argentina"),
    ("AT", "Austria"),
    ("BA", "Bosnia"),
    ("BE", "Belgium"),
    ("BG", "Bulgaria"),
    ("BO", "Bolivia"),
    ("BR", "Brazil"),
    ("CA", "Canada"),
    ("CH", "Switzerland"),
    ("CL", "Chile"),
    ("CO", "Colombia"),
    ("CR", "Costa Rica"),
    ("CZ", "Czechia"),
    ("DE", "Germany"),
    ("DK", "Denmark"),
    ("DO", "Dominican Republic"),
    ("EC", "Ecuador"),
    ("ES", "Spain"),
    (EX_YUGOSLAVIA, "Ex-Yugoslavia"),
    ("FI", "Finland"),
    ("FR", "France"),
    ("GB", "United Kingdom"),
    ("GR", "Greece"),
    ("HR", "Croatia"),
    ("HU", "Hungary"),
    ("IE", "Ireland"),
    ("IN", "India"),
    ("IT", "Italy"),
    (LATAM, "Latin America"),
    ("MX", "Mexico"),
    ("NL", "Netherlands"),
    ("NO", "Norway"),
    ("NZ", "New Zealand"),
    ("PE", "Peru"),
    ("PK", "Pakistan"),
    ("PL", "Poland"),
    ("PT", "Portugal"),
    ("RO", "Romania"),
    ("RS", "Serbia"),
    ("SE", "Sweden"),
    ("TR", "Turkey"),
    ("US", "United States"),
    ("UY", "Uruguay"),
    ("VE", "Venezuela"),
    ("ZA", "South Africa"),
    (ENGLISH, "English"),
    (OTHER, "Other"),
];

/// The **Region** a **Category** belongs to, or `None` when it cannot be told.
///
/// `None` is a real answer, not a failure: `ADULTS`, `NBA`, `PPV - SPORT` and
/// `PEACOCK` genuinely belong to no Region. The **Viewer** can assign one.
pub fn region_for_category(name: &str) -> Option<&'static str> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }

    // `XX - REST`
    if let Some((head, _)) = split_prefix(trimmed) {
        if head.len() == 2 {
            let upper = head.to_ascii_uppercase();
            if let Some((_, code)) = PREFIXES.iter().find(|(p, _)| *p == upper) {
                return Some(code);
            }
        }
        // `LAT - ...`, `ARABIC - ...`, `Polska - ...`
        if let Some(code) = lookup_name(head) {
            return Some(code);
        }
    }

    // A bare country name: `HOLLAND`, `SOUTH AFRICA`.
    lookup_name(trimmed)
}

pub fn region_label(code: &str) -> &str {
    LABELS
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, l)| *l)
        .unwrap_or(code)
}

fn split_prefix(name: &str) -> Option<(&str, &str)> {
    name.split_once(" - ").map(|(a, b)| (a.trim(), b.trim()))
}

fn lookup_name(candidate: &str) -> Option<&'static str> {
    // Unicode uppercase, not ASCII: `ESPAÑA` is a real Category name.
    let upper = candidate.trim().to_uppercase();
    NAMES
        .iter()
        .find(|(n, _)| *n == upper)
        .map(|(_, code)| *code)
}

/// Characters Providers pad **Divider** rows with.
const PADDING: &[char] = &['=', '*', '-', '_', '#', '~', '+'];

/// Whether a row is a **Divider** rather than a real **Channel** (ADR-0008).
///
/// Deliberately conservative: it takes a run of at least three padding
/// characters at the start *and* real text left over once the padding is
/// stripped. `======= BULGARIAN =======` matches. A Channel legitimately named
/// `E! HD` or `24/7 - COMEDY` does not.
pub fn is_divider(name: &str) -> bool {
    let trimmed = name.trim();
    let mut leading = trimmed.chars().take_while(|c| PADDING.contains(c));
    let Some(first) = leading.next() else {
        return false;
    };
    // All of the same character, at least three of them.
    let run = 1 + leading.take_while(|c| *c == first).count();
    if run < 3 {
        return false;
    }
    let stripped = trimmed.trim_matches(|c| PADDING.contains(&c) || c == ' ');
    !stripped.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_letter_prefixes_resolve() {
        assert_eq!(region_for_category("ES - FUTBOL"), Some("ES"));
        assert_eq!(region_for_category("IT - SKY ITALIA"), Some("IT"));
        assert_eq!(region_for_category("NZ - SPORTS"), Some("NZ"));
    }

    #[test]
    fn uk_and_gb_are_the_same_region() {
        // Categories say UK, Channel names say GB. One Region, or the filter
        // would hide half of Britain.
        assert_eq!(region_for_category("UK - SPORTS"), Some("GB"));
        assert_eq!(region_for_category("GB - NEWS"), Some("GB"));
    }

    #[test]
    fn ar_means_arabic_not_argentina() {
        // Every `AR - ` Category in a real catalogue is an Arabic-speaking
        // country: Morocco, Algeria, Qatar, UAE.
        assert_eq!(region_for_category("AR - MOROCCO"), Some(ARABIC));
        assert_eq!(region_for_category("AR - QATAR"), Some(ARABIC));
        // Argentina turns up under its own name instead.
        assert_eq!(region_for_category("ARGENTINA"), Some("AR"));
    }

    #[test]
    fn word_prefixes_resolve() {
        assert_eq!(region_for_category("LAT - MEXICO"), Some(LATAM));
        assert_eq!(region_for_category("LATINO - CINE"), Some(LATAM));
        assert_eq!(region_for_category("ARABIC - BEIN SPORTS"), Some(ARABIC));
        assert_eq!(region_for_category("Polska - Sportowe"), Some("PL"));
        assert_eq!(region_for_category("Bulgaria - Kids"), Some("BG"));
        assert_eq!(region_for_category("Greece - Movie"), Some("GR"));
    }

    #[test]
    fn bare_country_names_resolve() {
        assert_eq!(region_for_category("HOLLAND"), Some("NL"));
        assert_eq!(region_for_category("SOUTH AFRICA"), Some("ZA"));
        assert_eq!(region_for_category("CZECH"), Some("CZ"));
        assert_eq!(region_for_category("EX-YU"), Some(EX_YUGOSLAVIA));
        assert_eq!(region_for_category("BOSNA / HERSEK"), Some("BA"));
    }

    #[test]
    fn provider_misspellings_resolve() {
        assert_eq!(region_for_category("COLUMBIA"), Some("CO"));
        assert_eq!(region_for_category("HUNGARIA"), Some("HU"));
        assert_eq!(region_for_category("PAKISTANI"), Some("PK"));
    }

    #[test]
    fn language_groupings_resolve() {
        // Providers group VOD by language, not country.
        assert_eq!(region_for_category("ENGLISH - Drama"), Some(ENGLISH));
        assert_eq!(region_for_category("ESPAÑA - Comedia"), Some("ES"));
        assert_eq!(region_for_category("ESPANA - Comedia"), Some("ES"));
    }

    #[test]
    fn categories_with_no_region_return_none() {
        // These are real Categories. None of them is a place.
        for name in [
            "ADULTS",
            "NBA",
            "NFL",
            "MLS",
            "PEACOCK",
            "PPV - SPORT",
            "No Category",
        ] {
            assert_eq!(
                region_for_category(name),
                None,
                "{name} should have no Region"
            );
        }
    }

    #[test]
    fn other_is_a_real_region() {
        assert_eq!(region_label(OTHER), "Other");
    }

    #[test]
    fn labels_fall_back_to_the_code() {
        assert_eq!(region_label("ES"), "Spain");
        assert_eq!(region_label("GB"), "United Kingdom");
        assert_eq!(region_label(LATAM), "Latin America");
        assert_eq!(region_label("ZZ"), "ZZ");
    }

    #[test]
    fn dividers_are_recognised() {
        for name in [
            "======= BULGARIAN =======",
            "======== NETHERLANDS (HEVC) ========",
            "***** SWISS *****",
            "### GERMANY ###",
        ] {
            assert!(is_divider(name), "{name} should be a Divider");
        }
    }

    #[test]
    fn real_channels_are_not_dividers() {
        for name in [
            "|ES| LA 1",
            "E! HD",
            "24/7 - COMEDY",
            "-- Sky Sports", // only two padding chars
            "A&E HD",
            "===", // nothing left after stripping
        ] {
            assert!(!is_divider(name), "{name} must not be treated as a Divider");
        }
    }
}
