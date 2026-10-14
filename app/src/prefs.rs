//! Saved properties-panel state. Same storage as the theme preference.
//!
//! A missing value means the panel is hidden and unpinned, which is the
//! default. Unpinned panels close when the user clicks empty canvas.

pub const OPEN_KEY: &str = "pageform-properties-open";
pub const PIN_KEY: &str = "pageform-properties-pinned";
pub const RECENT_KEY: &str = "pageform-recent";

pub fn parse_recent(stored: Option<&str>) -> Vec<String> {
    stored
        .unwrap_or("")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(8)
        .map(str::to_string)
        .collect()
}

pub fn format_recent(paths: &[String]) -> String {
    paths.iter().take(8).cloned().collect::<Vec<_>>().join("\n")
}

pub fn is_open(stored: Option<&str>) -> bool {
    matches!(stored, Some("open"))
}

pub fn is_pinned(stored: Option<&str>) -> bool {
    matches!(stored, Some("pinned"))
}

pub fn open_value(open: bool) -> &'static str {
    if open {
        "open"
    } else {
        "closed"
    }
}

pub fn pin_value(pinned: bool) -> &'static str {
    if pinned {
        "pinned"
    } else {
        "unpinned"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_panel_state_is_hidden_and_unpinned() {
        assert!(!is_open(None));
        assert!(!is_open(Some("closed")));
        assert!(!is_open(Some("")));
        assert!(is_open(Some("open")));
        assert!(!is_pinned(None));
        assert!(!is_pinned(Some("unpinned")));
        assert!(is_pinned(Some("pinned")));
        assert_eq!(open_value(true), "open");
        assert_eq!(open_value(false), "closed");
        assert_eq!(pin_value(true), "pinned");
        assert_eq!(pin_value(false), "unpinned");
    }
}
