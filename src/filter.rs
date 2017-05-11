use syntax::ast::{Item, ItemKind};
use syntax::attr;

use regex::Regex;
use nom::IResult;

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
            if f.apply(item) {
                return true;
            }
        }
        false
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
        println!("Is {} a test? {}",
                 item.ident.name.as_str().as_ref() as &str,
                 attr::contains_name(&item.attrs, "test"));
        attr::contains_name(&item.attrs, "test")
    }
}

// A filter which returns true if an item is decorated with `#[bench]`.
struct BenchFilter;

impl BenchFilter {
    fn new() -> Box<Filter> {
        Box::new(BenchFilter {})
    }
}

impl Filter for BenchFilter {
    // Returns true if item is decorated with `#[bench]`.
    fn apply(&self, item: &Item) -> bool {
        println!("Is {} a bench? {}",
                 item.ident.name.as_str().as_ref() as &str,
                 attr::contains_name(&item.attrs, "bench"));
        attr::contains_name(&item.attrs, "bench")
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
    // require that the top-level expression be a call
    match call(filter.as_bytes()) {
        IResult::Done(_, out) => {
            println!("{:?}", out);
            expr_to_filter(&Expr::Call(out))
        }
        IResult::Error(err) => panic!("error parsing input: {:?}", err),
        IResult::Incomplete(left) => panic!("unparsed input: {:?}", left),
    }
}

fn expr_to_filter(expr: &Expr) -> Box<Filter> {
    match expr {
        &Expr::Quote(ref s) => panic!("unexpected string argument"),
        &Expr::Call(ref call) => {
            match call.name.as_str() {
                "test" => mk_no_arg_filter("test", &call.args, TestFilter::new()),
                "bench" => mk_no_arg_filter("bench", &call.args, BenchFilter::new()),
                "regex" => mk_regex_filter(&call.args),
                "fn" => mk_no_arg_filter("fn", &call.args, FnFilter::new()),
                "true" => mk_no_arg_filter("true", &call.args, AlwaysFilter::new()),
                "false" => mk_no_arg_filter("false", &call.args, NeverFilter::new()),
                "and" => mk_and_filter(&call.args),
                "or" => mk_or_filter(&call.args),
                "not" => mk_not_filter(&call.args),
                s => panic!("unrecognized function: {}", s),
            }
        }
    }
}

fn mk_regex_filter(args: &Vec<Expr>) -> Box<Filter> {
    if args.len() != 1 {
        panic!("regex() takes 1 argument");
    }
    if let Expr::Quote(ref s) = args[0] {
        match Regex::new(s.as_str()) {
            Ok(re) => RegexFilter::new(re),
            Err(err) => panic!("regex(): could not parse argument: {}", err),
        }
    } else {
        panic!("regex() only takes a string argument")
    }
}

fn mk_no_arg_filter(name: &str, args: &Vec<Expr>, filter: Box<Filter>) -> Box<Filter> {
    if args.len() != 0 {
        panic!("{}() takes no arguments", name);
    }
    filter
}

fn mk_and_filter(args: &Vec<Expr>) -> Box<Filter> {
    if args.len() == 0 {
        panic!("and() takes 1 or more arguments");
    }
    and(args_to_filters(args))
}

fn mk_or_filter(args: &Vec<Expr>) -> Box<Filter> {
    if args.len() == 0 {
        panic!("or() takes 1 or more arguments");
    }
    or(args_to_filters(args))
}

fn mk_not_filter(args: &Vec<Expr>) -> Box<Filter> {
    if args.len() != 1 {
        panic!("not() takes 1 argument");
    }
    not(expr_to_filter(&args[0]))
}

fn args_to_filters(args: &Vec<Expr>) -> Vec<Box<Filter>> {
    let mut v = Vec::new();
    for arg in args {
        v.push(expr_to_filter(arg));
    }
    v
}

#[derive(Debug)]
enum Expr {
    Quote(String),
    Call(Call),
}

#[derive(Debug)]
struct Call {
    name: String,
    args: Vec<Expr>,
}

fn bytes_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

// match a quoted string (a quote followed by non-quote characters followed by a quote)
named!(quote<String>, do_parse!(
    quote_: delimited!(char!('"'), take_until!("\""), char!('"')) >>
    (bytes_to_string(quote_))
));
// match a name (an alphabetic sequence)
// NOTE: The '^' at the beginning is VERY IMPORTANT - without it, we'd just consume and throw away
// any non-matching sequence of bytes until we found a match.
named!(name<String>, do_parse!(
    name_: re_bytes_find!("^[a-z]+") >>
    (bytes_to_string(name_))
));
// match an argument list (comma-separated expressions surrounded by parentheses)
named!(args<Vec<Expr> >, delimited!(
    ws!(char!('(')),
    separated_list!(ws!(char!(',')), expr),
    ws!(char!(')'))
));
// match a call (a name followed by an argument list)
named!(call<Call>, do_parse!(
    name_: name >>
    args_: args >>
    (Call{name: name_, args: args_})
));
// match an expression (either a call or a quote)
named!(expr<Expr>, alt_complete!(
    do_parse!(call_: ws!(call) >> (Expr::Call(call_))) |
    do_parse!(quote_: ws!(quote) >> (Expr::Quote(quote_)))
));
