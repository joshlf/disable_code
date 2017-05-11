use syntax::ast::{Item, ItemKind};
use syntax::attr;

use regex::Regex;

use std::env;

pub trait Filter {
    // Returns false if the item should be kept and true if it should be removed.
    fn apply(&self, &Item) -> bool;
}

// A filter which represents the AND of all of its sub-filters.
struct AllFilter(Vec<Box<Filter>>);

impl Filter for AllFilter {
    // Returns true only if all filters return true.
    fn apply(&self, item: &Item) -> bool {
        for f in self.0.iter() {
            if !f.apply(item) {
                return false;
            }
        }
        true
    }
}

// A filter which represents the OR of all of its sub-filters.
struct AnyFilter(Vec<Box<Filter>>);

impl Filter for AnyFilter {
    // Returns true if any filter returns true.
    fn apply(&self, item: &Item) -> bool {
        for f in self.0.iter() {
            if !f.apply(item) {
                return false;
            }
        }
        true
    }
}

// A filter which represents the negation of its sub-filter.
struct NotFilter(Box<Filter>);

impl Filter for NotFilter {
    // Returns the negation of whatever the wrapped filter returns.
    fn apply(&self, item: &Item) -> bool {
        !self.0.apply(item)
    }
}

// A filter which always returns true.
struct AlwaysFilter;

impl AlwaysFilter {
    fn new() -> Box<Filter> {
        Box::new(AlwaysFilter {})
    }
}

impl Filter for AlwaysFilter {
    // Returns true.
    fn apply(&self, _item: &Item) -> bool {
        true
    }
}

// A filter which always returns false.
struct NeverFilter;

impl NeverFilter {
    fn new() -> Box<Filter> {
        Box::new(NeverFilter {})
    }
}

impl Filter for NeverFilter {
    // Returns false.
    fn apply(&self, _item: &Item) -> bool {
        false
    }
}

// A filter which returns true if an item's name matches the specified regex.
struct RegexFilter(Regex);

impl RegexFilter {
    fn new(re: Regex) -> Box<Filter> {
        Box::new(RegexFilter(re))
    }
}

impl Filter for RegexFilter {
    // Returns true if the item's name matches the regex.
    fn apply(&self, item: &Item) -> bool {
        self.0.is_match(item.ident.name.as_str().as_ref())
    }
}

// A filter which returns true if an item is decorated with `#[test]`.
struct TestFilter;

impl TestFilter {
    fn new() -> Box<Filter> {
        Box::new(TestFilter {})
    }
}

impl Filter for TestFilter {
    // Returns true if item is decorated with `#[test]`.
    fn apply(&self, item: &Item) -> bool {
        attr::contains_name(&item.attrs, "test")
    }
}

// A filter which returns true if an item is a function declaration.
struct FnFilter;

impl FnFilter {
    fn new() -> Box<Filter> {
        Box::new(FnFilter {})
    }
}

impl Filter for FnFilter {
    // Returns true if item is a function declaration.
    fn apply(&self, item: &Item) -> bool {
        if let ItemKind::Fn(..) = item.node {
            true
        } else {
            false
        }
    }
}

// A filter which returns true if an item is the root module of a crate.
struct RootModFilter;

impl RootModFilter {
    fn new() -> Box<Filter> {
        Box::new(RootModFilter {})
    }
}

impl Filter for RootModFilter {
    // Returns true if item is the root module of a crate.
    fn apply(&self, item: &Item) -> bool {
        let &Item {
                 ref ident,
                 ref node,
                 ..
             } = item;
        if let &ItemKind::Mod(ref md) = node {
            ident.name.as_str() == ""
        } else {
            false
        }
    }
}

// Convenience function for constructing AllFilters.
fn and(filters: Vec<Box<Filter>>) -> Box<Filter> {
    let mut v = Vec::new();
    v.extend(filters);
    Box::new(AllFilter(v))
}

// Convenience function for constructing AnyFilters.
fn or(filters: Vec<Box<Filter>>) -> Box<Filter> {
    let mut v = Vec::new();
    v.extend(filters);
    Box::new(AnyFilter(v))
}

// Convenience function for constructing NotFilters.
fn not(filter: Box<Filter>) -> Box<Filter> {
    Box::new(NotFilter(filter))
}

const ENV_VAR_NAME: &str = "RUST_DISABLE_CODE_FILTER";

pub fn env_to_filter() -> Box<Filter> {
    match env::var(ENV_VAR_NAME) {
        // Never filter out the root module
        Ok(filter) => and(vec![not(RootModFilter::new()), parse_filter(filter)]),
        Err(_) => Box::new(NeverFilter {}),
    }
}

fn parse_filter(filter: String) -> Box<Filter> {
    match Regex::new(filter.as_str()) {
        Ok(re) => RegexFilter::new(re),
        // TODO: Print error message
        Err(_) => Box::new(NeverFilter {}),
    }
}
