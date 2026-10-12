use crate::models::ApplicationEntry;
use serde::Deserialize;
use std::sync::OnceLock;

const RATINGS_JSON: &str = include_str!("../assets/ratings.json");

#[derive(Debug, Deserialize)]
struct RatingsDatabase {
    entries: Vec<RatingEntry>,
    heuristics: RatingHeuristics,
}

#[derive(Debug, Deserialize)]
struct RatingEntry {
    pattern: String,
    rating: u8,
    #[serde(default = "default_field")]
    field: String,
}

fn default_field() -> String {
    "name".into()
}

#[derive(Debug, Deserialize)]
struct RatingHeuristics {
    system_component_rating: u8,
    protected_rating: u8,
    no_publisher_penalty: u8,
    default_rating: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RatingTier {
    Safe,
    Moderate,
    Risky,
    Unknown,
}

impl RatingTier {
    pub fn from_score(score: u8) -> Self {
        match score {
            80..=100 => Self::Safe,
            50..=79 => Self::Moderate,
            1..=49 => Self::Risky,
            _ => Self::Unknown,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Safe => "Safe",
            Self::Moderate => "Moderate",
            Self::Risky => "Risky",
            Self::Unknown => "Unknown",
        }
    }
}

fn database() -> &'static RatingsDatabase {
    static DB: OnceLock<RatingsDatabase> = OnceLock::new();
    DB.get_or_init(|| {
        serde_json::from_str(RATINGS_JSON).unwrap_or(RatingsDatabase {
            entries: Vec::new(),
            heuristics: RatingHeuristics {
                system_component_rating: 90,
                protected_rating: 85,
                no_publisher_penalty: 15,
                default_rating: 60,
            },
        })
    })
}

pub fn score_application(app: &ApplicationEntry) -> u8 {
    let db = database();

    if app.is_system_component {
        return db.heuristics.system_component_rating;
    }
    if app.is_protected {
        return db.heuristics.protected_rating;
    }

    let name_lower = app.display_name.to_lowercase();
    let publisher_lower = app.publisher.as_ref().map(|p| p.to_lowercase());

    let mut best = db.heuristics.default_rating;

    for entry in &db.entries {
        let haystack = match entry.field.as_str() {
            "publisher" => publisher_lower.as_deref().unwrap_or(""),
            _ => &name_lower,
        };
        if haystack.contains(&entry.pattern.to_lowercase()) {
            if entry.rating < db.heuristics.default_rating {
                best = best.min(entry.rating);
            } else {
                best = best.max(entry.rating);
            }
        }
    }

    if app.publisher.is_none() {
        best = best.saturating_sub(db.heuristics.no_publisher_penalty);
    }

    if app.is_orphan {
        best = best.saturating_sub(10);
    }

    best.clamp(1, 100)
}

pub fn apply_ratings(apps: &mut [ApplicationEntry]) {
    for app in apps.iter_mut() {
        app.rating = Some(score_application(app));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, UninstallerType};

    fn sample(name: &str, publisher: Option<&str>) -> ApplicationEntry {
        ApplicationEntry {
            id: name.into(),
            display_name: name.into(),
            version: None,
            publisher: publisher.map(str::to_string),
            source: AppSource::Registry,
            install_location: None,
            uninstall_string: Some("cmd".into()),
            quiet_uninstall_string: None,
            uninstaller_type: UninstallerType::Unknown,
            size_estimate: None,
            install_date: None,
            is_protected: false,
            is_system_component: false,
            is_orphan: false,
            rating: None,
        }
    }

    #[test]
    fn known_bloatware_scores_low() {
        let app = sample("BonziBUDDY 4.0", Some("Bonzi Software"));
        assert!(score_application(&app) < 50);
        assert_eq!(
            RatingTier::from_score(score_application(&app)),
            RatingTier::Risky
        );
    }

    #[test]
    fn trusted_app_scores_high() {
        let app = sample("7-Zip 24.00", Some("Igor Pavlov"));
        assert!(score_application(&app) >= 80);
    }

    #[test]
    fn system_component_scores_high() {
        let mut app = sample("Windows Feature", Some("Microsoft"));
        app.is_system_component = true;
        assert!(score_application(&app) >= 80);
    }
}
