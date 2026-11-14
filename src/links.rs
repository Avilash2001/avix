//! Where Avix points people: the project, its author, and ways to support or contribute.
//! Every outside link the dashboard shows lives here.

pub const AUTHOR: &str = "Avilash Ghosh";

pub const REPO_URL: &str = "https://github.com/Avilash2001/avix";
pub const ISSUES_URL: &str = "https://github.com/Avilash2001/avix/issues/new/choose";
pub const CONTRIBUTING_URL: &str = "https://github.com/Avilash2001/avix/blob/main/CONTRIBUTING.md";
pub const PRIVACY_URL: &str = "https://github.com/Avilash2001/avix/blob/main/PRIVACY.md";

/// The author's profiles, shown in Settings → About: (site, handle, url).
pub const SOCIALS: &[(&str, &str, &str)] = &[
    ("GitHub", "Avilash2001", "https://github.com/Avilash2001"),
    ("X", "@AvilashGhosh6", "https://x.com/AvilashGhosh6"),
];

/// Buy Me a Coffee username (the part after buymeacoffee.com/). Empty hides every coffee button.
pub const BUY_ME_A_COFFEE: &str = "avilashghop";

pub fn coffee_url() -> Option<String> {
    (!BUY_ME_A_COFFEE.is_empty()).then(|| format!("https://buymeacoffee.com/{BUY_ME_A_COFFEE}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_links_point_at_this_repo() {
        let repo = format!("https://github.com/{}", crate::update::REPO);
        assert_eq!(REPO_URL, repo);
        for url in [ISSUES_URL, CONTRIBUTING_URL, PRIVACY_URL] {
            assert!(url.starts_with(&format!("{repo}/")), "{url}");
        }
    }

    #[test]
    fn every_link_is_https() {
        // open_url ignores anything else.
        let all = [REPO_URL, ISSUES_URL, CONTRIBUTING_URL, PRIVACY_URL].into_iter().chain(SOCIALS.iter().map(|s| s.2));
        for url in all {
            assert!(url.starts_with("https://"), "{url}");
        }
        assert!(!BUY_ME_A_COFFEE.contains('/'), "just the username, not a URL");
    }
}
