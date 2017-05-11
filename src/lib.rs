#![crate_type="dylib"]
#![feature(plugin_registrar, rustc_private)]

mod filter;

#[macro_use]
extern crate nom;
extern crate regex;
extern crate syntax;
extern crate syntax_pos;
extern crate rustc;
extern crate rustc_plugin;

use syntax::feature_gate::AttributeType;
use syntax::symbol::Symbol;
use syntax::ext::quote::rt::Span;
use syntax::ast::{MetaItem, Item, ItemKind, NodeId, Visibility};
use syntax::ext::base::{ExtCtxt, Annotatable};
use syntax::ext::base::SyntaxExtension;
use syntax::ptr::P;
use syntax_pos::hygiene::SyntaxContext;
use syntax_pos::BytePos;
use syntax_pos::symbol::Ident;
use rustc_plugin::Registry;

use filter::Filter;

fn modify_ast(cx: &mut ExtCtxt,
              span: Span,
              ast: &MetaItem,
              annotatable: Annotatable)
              -> Annotatable {
    if let Annotatable::Item(item) = annotatable {
        let mut it = item.unwrap();
        // We should never be filtering out the root module
        assert!(!delete_item(filter::env_to_filter().as_ref(), &mut it));
        Annotatable::Item(P(it))
    } else {
        // TODO: Emit warning about non-crate attribute
        annotatable
    }
}

// Deletes any items that should be deleted, and returns true if its argument should be deleted.
fn delete_item(filter: &Filter, item: &mut Item) -> bool {
    if filter.apply(item) {
        return true;
    }

    match &mut item.node {
        &mut ItemKind::Mod(ref mut md) => {
            let mut to_delete = Vec::new();
            for i in 0..md.items.len() {
                // We can't mutate the item directly because P (libsyntax's owned pointer type)
                // doesn't allow mutation of its referent. We also can't take ownership of its
                // referent because that would consitute moving a borrowed value (since we only
                // have a mutable reference). Thus, we do this silly song and dance of creating a
                // dummy P<Item>, swap it with the item, do what we need to on the dummy, and then
                // swap it back.

                let mut item = md.items.get_mut(i).unwrap();
                let mut dummy = P(dummy_item());
                use std::mem::swap;
                swap(item, &mut dummy);

                let mut it = dummy.unwrap();
                let delete = delete_item(filter, &mut it);
                swap(item, &mut P(it));

                if delete {
                    to_delete.push(i);
                }
            }

            let mut offset = 0;
            for i in to_delete {
                md.items.remove(i - offset);
                offset += 1;
            }

            false
        }
        _ => false,
    }
}

// Returns an arbitrary Item. It should not be used for anything other than temporarily taking the
// place of other Items.
fn dummy_item() -> Item {
    Item {
        ident: Ident::with_empty_ctxt(Symbol::intern("")),
        attrs: Vec::new(),
        id: NodeId::new(0),
        node: ItemKind::ExternCrate(None),
        vis: Visibility::Public,
        span: Span {
            lo: BytePos(0),
            hi: BytePos(0),
            ctxt: SyntaxContext::empty(),
        },
    }
}

#[plugin_registrar]
pub fn plugin_registrar(reg: &mut Registry) {
    reg.register_attribute(String::from("disable_code"), AttributeType::CrateLevel);
    reg.register_syntax_extension(Symbol::intern("disable_code"),
                                  SyntaxExtension::MultiModifier(Box::new(modify_ast)));
}
