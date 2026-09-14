// crates/talaria-api/src/person_ingest/crawl_queue.rs
//! Prioritized crawl queue for person ingest.
//!
//! Ensures core subject pages (main Wikipedia, Wikidata) are processed first,
//! then high-signal links (birth/death places, battles, treaties), then
//! lower-priority expansion pages.

use std::cmp::Ordering;
use std::collections::HashSet;

use talaria_sources::{is_followable_map_title, is_high_value_link_title, is_life_trace_link_title};

/// Priority tiers for crawl items. Lower numeric value = higher priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrawlPriority {
    /// Subject's own Wikipedia pages (all languages) and Wikidata statements.
    /// Processed immediately to populate map/timeline with core events.
    Core = 0,
    /// Direct Wikidata links: birth/death places, works by subject, key institutions.
    /// Also includes explicitly identified life-trace pages (residence, childhood, etc).
    High = 1,
    /// Battle pages, treaties, places mentioned with dates in text.
    /// Standard "followable map titles" from page links.
    Medium = 2,
    /// Generic links, category neighbors, depth-2/3 pages without strong signals.
    /// Processed last to fill remaining document budget.
    Low = 3,
}

impl CrawlPriority {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Core,
            1 => Self::High,
            2 => Self::Medium,
            _ => Self::Low,
        }
    }

    pub fn phase_name(self) -> &'static str {
        match self {
            Self::Core => "core_extract",
            Self::High => "high_priority_links",
            Self::Medium => "expand_links",
            Self::Low => "expand_links",
        }
    }
}

impl Ord for CrawlPriority {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_u8().cmp(&other.as_u8())
    }
}

impl PartialOrd for CrawlPriority {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Source that discovered this crawl item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrawlSource {
    /// Subject's main Wikipedia page (primary entry point).
    SubjectWikipedia,
    /// Wikidata structured statements.
    Wikidata,
    /// WDQS participation events (battles, conferences, etc).
    WdqsEvents,
    /// Page link discovered in another page's content.
    PageLink { from_title: String },
    /// Curated seed list file.
    SeedList,
}

impl CrawlSource {
    pub fn display(&self) -> String {
        match self {
            Self::SubjectWikipedia => "subject_wikipedia".into(),
            Self::Wikidata => "wikidata".into(),
            Self::WdqsEvents => "wdqs_events".into(),
            Self::PageLink { from_title } => format!("page_link:{}", from_title),
            Self::SeedList => "seed_list".into(),
        }
    }
}

/// A page queued for crawling with priority metadata.
#[derive(Debug, Clone)]
pub struct CrawlItem {
    pub title: String,
    pub priority: CrawlPriority,
    pub source: CrawlSource,
    pub depth: u8,
}

impl CrawlItem {
    pub fn new(title: impl Into<String>, priority: CrawlPriority, source: CrawlSource) -> Self {
        Self {
            title: title.into(),
            priority,
            source,
            depth: 0,
        }
    }

    pub fn with_depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }
}

/// Priority-ordered crawl queue that processes high-priority pages first.
///
/// Unlike a simple FIFO queue, this ensures core subject pages are extracted
/// before bulk expansion links, providing faster initial map/timeline population.
#[derive(Debug, Default)]
pub struct PriorityCrawlQueue {
    items: Vec<CrawlItem>,
    seen: HashSet<String>,
    next_index: usize,
}

impl PriorityCrawlQueue {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an item if not already seen. Returns true if added.
    pub fn push(&mut self, item: CrawlItem) -> bool {
        let key = item.title.to_lowercase();
        if self.seen.contains(&key) {
            return false;
        }
        self.seen.insert(key);
        self.items.push(item);
        true
    }

    /// Add multiple items, filtering duplicates.
    pub fn extend(&mut self, items: impl IntoIterator<Item = CrawlItem>) {
        for item in items {
            self.push(item);
        }
    }

    /// Mark a title as seen without adding it to the queue.
    pub fn mark_seen(&mut self, title: &str) {
        self.seen.insert(title.to_lowercase());
    }

    /// Check if a title has been seen.
    pub fn is_seen(&self, title: &str) -> bool {
        self.seen.contains(&title.to_lowercase())
    }

    /// Sort items by priority (stable sort to preserve discovery order within tiers).
    /// Call before iteration to ensure priority ordering.
    pub fn sort_by_priority(&mut self) {
        self.items[self.next_index..].sort_by(|a, b| {
            a.priority
                .cmp(&b.priority)
                .then_with(|| a.depth.cmp(&b.depth))
        });
    }

    /// Get the next item to process without removing it.
    pub fn peek(&self) -> Option<&CrawlItem> {
        self.items.get(self.next_index)
    }

    /// Advance to the next item.
    pub fn advance(&mut self) {
        if self.next_index < self.items.len() {
            self.next_index += 1;
        }
    }

    /// Get the next item and advance.
    pub fn pop(&mut self) -> Option<CrawlItem> {
        if self.next_index < self.items.len() {
            let item = self.items[self.next_index].clone();
            self.next_index += 1;
            Some(item)
        } else {
            None
        }
    }

    /// Number of items remaining to process.
    pub fn remaining(&self) -> usize {
        self.items.len().saturating_sub(self.next_index)
    }

    /// Total items added (processed + remaining).
    pub fn total_added(&self) -> usize {
        self.items.len()
    }

    /// Number of items already processed.
    pub fn processed(&self) -> usize {
        self.next_index
    }

    /// Check if empty.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Current priority tier being processed.
    pub fn current_priority(&self) -> Option<CrawlPriority> {
        self.peek().map(|item| item.priority)
    }

    /// Count items remaining by priority tier.
    pub fn count_by_priority(&self) -> [u32; 4] {
        let mut counts = [0u32; 4];
        for item in &self.items[self.next_index..] {
            counts[item.priority.as_u8() as usize] += 1;
        }
        counts
    }

    /// Iterator over remaining items (does not consume).
    pub fn iter_remaining(&self) -> impl Iterator<Item = &CrawlItem> {
        self.items[self.next_index..].iter()
    }
}

/// Determine priority for a page link based on its title and context.
pub fn classify_link_priority(
    title: &str,
    subject: &str,
    from_wikidata: bool,
    from_wdqs: bool,
) -> CrawlPriority {
    // Subject's own page is always Core
    if title.eq_ignore_ascii_case(subject) {
        return CrawlPriority::Core;
    }

    // Wikidata direct links (birth place, death place, etc.) are High priority
    if from_wikidata {
        return CrawlPriority::High;
    }

    // WDQS events (battles, conferences the subject participated in) are High
    if from_wdqs {
        return CrawlPriority::High;
    }

    // Life trace pages (house, residence, childhood, etc.) are High
    if is_life_trace_link_title(title) {
        return CrawlPriority::High;
    }

    // Standard followable map titles (battles, treaties, palaces) are Medium
    if is_high_value_link_title(title) || is_followable_map_title(title) {
        return CrawlPriority::Medium;
    }

    // Everything else is Low
    CrawlPriority::Low
}

/// Create crawl items from Wikipedia page links.
pub fn crawl_items_from_page_links(
    links: &[String],
    from_title: &str,
    subject: &str,
    cap: u32,
) -> Vec<CrawlItem> {
    let mut out = Vec::new();
    for title in links {
        if out.len() as u32 >= cap {
            break;
        }
        if !is_followable_map_title(title) {
            continue;
        }
        let priority = classify_link_priority(title, subject, false, false);
        out.push(CrawlItem::new(
            title.clone(),
            priority,
            CrawlSource::PageLink {
                from_title: from_title.to_string(),
            },
        ));
    }
    out
}

/// Create crawl items from WDQS events.
pub fn crawl_items_from_wdqs(
    events: &[talaria_sources::wdqs::WdqsEvent],
    subject: &str,
    cap: u32,
) -> Vec<CrawlItem> {
    let mut out = Vec::new();
    for ev in events {
        if out.len() as u32 >= cap {
            break;
        }
        if !is_followable_map_title(&ev.label) {
            continue;
        }
        // WDQS participation events are high priority - they're direct subject involvement
        let priority = classify_link_priority(&ev.label, subject, false, true);
        out.push(CrawlItem::new(
            ev.label.clone(),
            priority,
            CrawlSource::WdqsEvents,
        ));
    }
    out
}

/// Create crawl items from a seed list file.
pub fn crawl_items_from_seed_list(titles: Vec<String>, subject: &str) -> Vec<CrawlItem> {
    titles
        .into_iter()
        .map(|title| {
            let priority = classify_link_priority(&title, subject, false, false);
            CrawlItem::new(title, priority, CrawlSource::SeedList)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_ordering_is_correct() {
        assert!(CrawlPriority::Core < CrawlPriority::High);
        assert!(CrawlPriority::High < CrawlPriority::Medium);
        assert!(CrawlPriority::Medium < CrawlPriority::Low);
    }

    #[test]
    fn queue_processes_core_before_medium() {
        let mut queue = PriorityCrawlQueue::new();
        queue.push(CrawlItem::new(
            "Battle of Waterloo",
            CrawlPriority::Medium,
            CrawlSource::PageLink {
                from_title: "Napoleon".into(),
            },
        ));
        queue.push(CrawlItem::new(
            "Napoleon",
            CrawlPriority::Core,
            CrawlSource::SubjectWikipedia,
        ));
        queue.push(CrawlItem::new(
            "Ajaccio",
            CrawlPriority::High,
            CrawlSource::Wikidata,
        ));

        queue.sort_by_priority();

        let first = queue.pop().unwrap();
        assert_eq!(first.title, "Napoleon");
        assert_eq!(first.priority, CrawlPriority::Core);

        let second = queue.pop().unwrap();
        assert_eq!(second.title, "Ajaccio");
        assert_eq!(second.priority, CrawlPriority::High);

        let third = queue.pop().unwrap();
        assert_eq!(third.title, "Battle of Waterloo");
        assert_eq!(third.priority, CrawlPriority::Medium);
    }

    #[test]
    fn queue_deduplicates_by_title() {
        let mut queue = PriorityCrawlQueue::new();
        assert!(queue.push(CrawlItem::new(
            "Battle of Austerlitz",
            CrawlPriority::Medium,
            CrawlSource::SeedList
        )));
        assert!(!queue.push(CrawlItem::new(
            "battle of austerlitz",
            CrawlPriority::High,
            CrawlSource::Wikidata
        )));
        assert_eq!(queue.remaining(), 1);
    }

    #[test]
    fn mark_seen_prevents_addition() {
        let mut queue = PriorityCrawlQueue::new();
        queue.mark_seen("Treaty of Tilsit");
        assert!(!queue.push(CrawlItem::new(
            "Treaty of Tilsit",
            CrawlPriority::Medium,
            CrawlSource::SeedList
        )));
        assert!(queue.is_empty());
    }

    #[test]
    fn classify_link_priority_subject_is_core() {
        assert_eq!(
            classify_link_priority("Napoleon", "Napoleon", false, false),
            CrawlPriority::Core
        );
    }

    #[test]
    fn classify_link_priority_wikidata_is_high() {
        assert_eq!(
            classify_link_priority("Ajaccio", "Napoleon", true, false),
            CrawlPriority::High
        );
    }

    #[test]
    fn classify_link_priority_wdqs_is_high() {
        assert_eq!(
            classify_link_priority("Battle of Austerlitz", "Napoleon", false, true),
            CrawlPriority::High
        );
    }

    #[test]
    fn classify_link_priority_life_trace_is_high() {
        assert_eq!(
            classify_link_priority("Maison de George Sand", "George Sand", false, false),
            CrawlPriority::High
        );
    }

    #[test]
    fn classify_link_priority_battle_is_medium() {
        assert_eq!(
            classify_link_priority("Battle of Waterloo", "Napoleon", false, false),
            CrawlPriority::Medium
        );
    }

    #[test]
    fn classify_link_priority_generic_is_low() {
        assert_eq!(
            classify_link_priority("Some Random Page", "Napoleon", false, false),
            CrawlPriority::Low
        );
    }

    #[test]
    fn count_by_priority_reflects_remaining() {
        let mut queue = PriorityCrawlQueue::new();
        queue.push(CrawlItem::new("A", CrawlPriority::Core, CrawlSource::SubjectWikipedia));
        queue.push(CrawlItem::new("B", CrawlPriority::High, CrawlSource::Wikidata));
        queue.push(CrawlItem::new("C", CrawlPriority::Medium, CrawlSource::SeedList));
        queue.push(CrawlItem::new("D", CrawlPriority::Medium, CrawlSource::SeedList));
        queue.push(CrawlItem::new("E", CrawlPriority::Low, CrawlSource::SeedList));

        let counts = queue.count_by_priority();
        assert_eq!(counts, [1, 1, 2, 1]); // Core, High, Medium, Low

        queue.sort_by_priority();
        queue.pop(); // Remove Core
        let counts_after = queue.count_by_priority();
        assert_eq!(counts_after, [0, 1, 2, 1]);
    }

    #[test]
    fn stable_sort_preserves_discovery_order_within_tier() {
        let mut queue = PriorityCrawlQueue::new();
        queue.push(CrawlItem::new("First Medium", CrawlPriority::Medium, CrawlSource::SeedList));
        queue.push(CrawlItem::new("Second Medium", CrawlPriority::Medium, CrawlSource::SeedList));
        queue.push(CrawlItem::new("Third Medium", CrawlPriority::Medium, CrawlSource::SeedList));
        queue.push(CrawlItem::new("Core", CrawlPriority::Core, CrawlSource::SubjectWikipedia));

        queue.sort_by_priority();

        assert_eq!(queue.pop().unwrap().title, "Core");
        assert_eq!(queue.pop().unwrap().title, "First Medium");
        assert_eq!(queue.pop().unwrap().title, "Second Medium");
        assert_eq!(queue.pop().unwrap().title, "Third Medium");
    }
}
