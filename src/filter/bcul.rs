use crate::models::ApplicationEntry;
use anyhow::{Context, Result};
use regex::Regex;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonMethod {
    Equals,
    Any,
    StartsWith,
    EndsWith,
    Contains,
    Regex,
}

impl ComparisonMethod {
    fn from_str(value: &str) -> Self {
        match value {
            "Equals" => Self::Equals,
            "StartsWith" => Self::StartsWith,
            "EndsWith" => Self::EndsWith,
            "Contains" => Self::Contains,
            "Regex" => Self::Regex,
            _ => Self::Any,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FilterCondition {
    pub filter_text: String,
    pub comparison_method: ComparisonMethod,
    pub target_property: String,
    pub invert_results: bool,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct BculFilter {
    pub _name: String,
    pub exclude: bool,
    pub enabled: bool,
    pub conditions: Vec<FilterCondition>,
}

#[derive(Debug, Clone)]
pub struct BculList {
    pub enabled: bool,
    pub filters: Vec<BculFilter>,
}

impl BculList {
    pub fn from_file(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        parse_bcul_xml(&contents)
    }

    pub fn test_entry(&self, app: &ApplicationEntry) -> Option<bool> {
        if !self.enabled || self.filters.is_empty() {
            return None;
        }

        let excludes: Vec<_> = self.filters.iter().filter(|f| f.exclude).collect();
        let includes: Vec<_> = self.filters.iter().filter(|f| !f.exclude).collect();

        for filter in &excludes {
            if filter.test_entry(app) == Some(true) {
                return Some(false);
            }
        }

        let excluded = if excludes.is_empty() {
            None
        } else {
            Some(false)
        };

        for filter in &includes {
            if filter.test_entry(app) == Some(true) {
                return Some(true);
            }
        }

        let included = if includes.is_empty() {
            None
        } else {
            Some(false)
        };

        if included.is_none() {
            return excluded.map(|_| true);
        }
        included
    }

    pub fn matching_indices(&self, apps: &[ApplicationEntry]) -> Vec<usize> {
        apps.iter()
            .enumerate()
            .filter(|(_, app)| self.test_entry(app).unwrap_or(false))
            .map(|(i, _)| i)
            .collect()
    }
}

impl BculFilter {
    fn test_entry(&self, app: &ApplicationEntry) -> Option<bool> {
        if !self.enabled {
            return None;
        }

        let active: Vec<_> = self
            .conditions
            .iter()
            .filter(|c| c.enabled && !c.filter_text.is_empty())
            .collect();
        if active.is_empty() {
            return None;
        }

        let results: Vec<bool> = active.iter().filter_map(|c| c.test_entry(app)).collect();
        if results.is_empty() {
            return None;
        }
        Some(results.iter().all(|&r| r))
    }
}

impl FilterCondition {
    fn test_entry(&self, app: &ApplicationEntry) -> Option<bool> {
        if !self.enabled {
            return None;
        }

        let targets = self.target_values(app);
        if targets.is_empty() {
            return None;
        }

        let mut result = None;
        for target in targets {
            if target.is_empty() {
                continue;
            }
            let matched = self.match_target(&target);
            if matched == Some(true) {
                return Some(!self.invert_results);
            }
            result = matched;
        }

        result.map(|r| if self.invert_results { !r } else { r })
    }

    fn target_values(&self, app: &ApplicationEntry) -> Vec<String> {
        if self.target_property.is_empty()
            || self.target_property.eq_ignore_ascii_case("All")
            || self.target_property.eq_ignore_ascii_case("Any")
        {
            return vec![
                app.display_name.clone(),
                app.publisher.clone().unwrap_or_default(),
                app.version.clone().unwrap_or_default(),
                app.id.clone(),
                app.source.label().to_string(),
            ];
        }

        let value = match self.target_property.to_ascii_lowercase().as_str() {
            "displayname" | "name" => Some(app.display_name.clone()),
            "publisher" => app.publisher.clone(),
            "version" | "displayversion" => app.version.clone(),
            "id" | "registrykeyname" => Some(app.id.clone()),
            "source" | "abouturl" => Some(app.source.label().to_string()),
            "installlocation" => app.install_location.clone(),
            _ => None,
        };
        value.map(|v| vec![v]).unwrap_or_default()
    }

    fn match_target(&self, target: &str) -> Option<bool> {
        let filter = &self.filter_text;
        let result = match self.comparison_method {
            ComparisonMethod::Equals => target.eq_ignore_ascii_case(filter),
            ComparisonMethod::StartsWith => target
                .to_ascii_lowercase()
                .starts_with(&filter.to_ascii_lowercase()),
            ComparisonMethod::EndsWith => target
                .to_ascii_lowercase()
                .ends_with(&filter.to_ascii_lowercase()),
            ComparisonMethod::Contains => target
                .to_ascii_lowercase()
                .contains(&filter.to_ascii_lowercase()),
            ComparisonMethod::Regex => Regex::new(filter)
                .map(|re| re.is_match(target))
                .unwrap_or(false),
            ComparisonMethod::Any => filter.split_whitespace().any(|token| {
                target
                    .to_ascii_lowercase()
                    .contains(&token.to_ascii_lowercase())
            }),
        };
        Some(result)
    }
}

fn parse_bcul_xml(xml: &str) -> Result<BculList> {
    let trimmed = xml.trim_start_matches('\u{feff}');
    let doc = roxmltree::Document::parse(trimmed).context("invalid .bcul XML")?;
    let root = doc.root_element();

    if root.tag_name().name() != "UninstallList" {
        anyhow::bail!(
            "expected root element UninstallList, found {}",
            root.tag_name().name()
        );
    }

    let enabled = parse_bool_child(&root, "Enabled", true);
    let mut filters = Vec::new();

    for filter_node in root
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Filter")
    {
        filters.push(parse_filter(filter_node)?);
    }

    if filters.is_empty() {
        for filters_node in root
            .children()
            .filter(|n| n.is_element() && n.tag_name().name() == "Filters")
        {
            for filter_node in filters_node
                .children()
                .filter(|n| n.is_element() && n.tag_name().name() == "Filter")
            {
                filters.push(parse_filter(filter_node)?);
            }
        }
    }

    if filters.is_empty() {
        anyhow::bail!("uninstall list contains no filters");
    }

    Ok(BculList { enabled, filters })
}

fn parse_filter(node: roxmltree::Node<'_, '_>) -> Result<BculFilter> {
    let name = text_child(node, "Name").unwrap_or_else(|| "Filter".into());
    let exclude = parse_bool_child(&node, "Exclude", false);
    let enabled = parse_bool_child(&node, "Enabled", true);
    let mut conditions = Vec::new();

    for child in node.children().filter(|n| n.is_element()) {
        match child.tag_name().name() {
            "ComparisonEntries" | "Conditions" => {
                for cond_node in child
                    .children()
                    .filter(|n| n.is_element() && n.tag_name().name() == "FilterCondition")
                {
                    conditions.push(parse_condition(cond_node));
                }
            }
            "FilterCondition" => conditions.push(parse_condition(child)),
            _ => {}
        }
    }

    if conditions.is_empty() {
        if let Some(text) = text_child(node, "FilterText") {
            conditions.push(FilterCondition {
                filter_text: text,
                comparison_method: ComparisonMethod::Any,
                target_property: String::new(),
                invert_results: false,
                enabled: true,
            });
        }
    }

    Ok(BculFilter {
        _name: name,
        exclude,
        enabled,
        conditions,
    })
}

fn parse_condition(node: roxmltree::Node<'_, '_>) -> FilterCondition {
    let filter_text = text_child(node, "FilterText").unwrap_or_default();
    let method = text_child(node, "ComparisonMethod")
        .map(|s| ComparisonMethod::from_str(&s))
        .unwrap_or(ComparisonMethod::Any);
    let target_property = text_child(node, "TargetPropertyId")
        .or_else(|| text_child(node, "TargetProperty"))
        .unwrap_or_default();
    let invert_results = parse_bool_child(&node, "InvertResults", false);
    let enabled = parse_bool_child(&node, "Enabled", true);

    FilterCondition {
        filter_text,
        comparison_method: method,
        target_property,
        invert_results,
        enabled,
    }
}

fn text_child(node: roxmltree::Node<'_, '_>, tag: &str) -> Option<String> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == tag)
        .and_then(|n| n.text())
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

fn parse_bool_child(node: &roxmltree::Node<'_, '_>, tag: &str, default: bool) -> bool {
    text_child(*node, tag)
        .and_then(|s| s.parse::<bool>().ok())
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{AppSource, UninstallerType};

    fn app(name: &str) -> ApplicationEntry {
        ApplicationEntry {
            id: name.into(),
            display_name: name.into(),
            version: None,
            publisher: None,
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
    fn parses_include_filter() {
        let xml = r#"<?xml version="1.0"?>
<UninstallList>
  <Enabled>true</Enabled>
  <Filters>
    <Filter>
      <Name>Test</Name>
      <Exclude>false</Exclude>
      <Enabled>true</Enabled>
      <ComparisonEntries>
        <FilterCondition>
          <FilterText>Bonzi</FilterText>
          <ComparisonMethod>Contains</ComparisonMethod>
          <TargetPropertyId>DisplayName</TargetPropertyId>
          <Enabled>true</Enabled>
        </FilterCondition>
      </ComparisonEntries>
    </Filter>
  </Filters>
</UninstallList>"#;
        let list = parse_bcul_xml(xml).unwrap();
        assert!(list.test_entry(&app("BonziBUDDY")).unwrap());
        assert!(!list.test_entry(&app("Firefox")).unwrap_or(false));
    }

    #[test]
    fn exclude_filter_rejects_match() {
        let xml = r#"<?xml version="1.0"?>
<UninstallList>
  <Filters>
    <Filter>
      <Exclude>true</Exclude>
      <ComparisonEntries>
        <FilterCondition>
          <FilterText>Firefox</FilterText>
          <ComparisonMethod>Contains</ComparisonMethod>
        </FilterCondition>
      </ComparisonEntries>
    </Filter>
    <Filter>
      <Exclude>false</Exclude>
      <ComparisonEntries>
        <FilterCondition>
          <FilterText>zip</FilterText>
          <ComparisonMethod>Contains</ComparisonMethod>
        </FilterCondition>
      </ComparisonEntries>
    </Filter>
  </Filters>
</UninstallList>"#;
        let list = parse_bcul_xml(xml).unwrap();
        assert_eq!(list.test_entry(&app("Mozilla Firefox")), Some(false));
        assert!(list.test_entry(&app("7-Zip")).unwrap());
    }
}
