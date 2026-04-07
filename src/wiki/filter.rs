use crate::types::WikiPage;

/// Filter pages based on include/exclude slug lists.
pub fn filter_pages(
    pages: Vec<WikiPage>,
    include: Option<&[String]>,
    exclude: Option<&[String]>,
) -> Vec<WikiPage> {
    pages
        .into_iter()
        .filter(|page| {
            // If include list is specified, page must match
            if let Some(includes) = include {
                if !includes.iter().any(|slug| page.slug.starts_with(slug)) {
                    return false;
                }
            }
            // If exclude list is specified, page must not match
            if let Some(excludes) = exclude {
                if excludes.iter().any(|slug| page.slug.starts_with(slug)) {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pages() -> Vec<WikiPage> {
        vec![
            WikiPage {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
                content: "overview".to_string(),
            },
            WikiPage {
                slug: "1.1-architecture".to_string(),
                title: "Architecture".to_string(),
                depth: 1,
                content: "arch".to_string(),
            },
            WikiPage {
                slug: "2-guide".to_string(),
                title: "Guide".to_string(),
                depth: 0,
                content: "guide".to_string(),
            },
            WikiPage {
                slug: "3-glossary".to_string(),
                title: "Glossary".to_string(),
                depth: 0,
                content: "glossary".to_string(),
            },
        ]
    }

    #[test]
    fn test_filter_no_filter() {
        let pages = filter_pages(make_pages(), None, None);
        assert_eq!(pages.len(), 4);
    }

    #[test]
    fn test_filter_include() {
        let include = vec!["1".to_string(), "2".to_string()];
        let pages = filter_pages(make_pages(), Some(&include), None);
        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].slug, "1-overview");
        assert_eq!(pages[1].slug, "1.1-architecture");
        assert_eq!(pages[2].slug, "2-guide");
    }

    #[test]
    fn test_filter_exclude() {
        let exclude = vec!["3".to_string()];
        let pages = filter_pages(make_pages(), None, Some(&exclude));
        assert_eq!(pages.len(), 3);
        assert!(pages.iter().all(|p| p.slug != "3-glossary"));
    }

    #[test]
    fn test_filter_include_and_exclude() {
        let include = vec!["1".to_string()];
        let exclude = vec!["1.1".to_string()];
        let pages = filter_pages(make_pages(), Some(&include), Some(&exclude));
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].slug, "1-overview");
    }

    #[test]
    fn test_filter_no_match() {
        let include = vec!["99".to_string()];
        let pages = filter_pages(make_pages(), Some(&include), None);
        assert!(pages.is_empty());
    }
}
