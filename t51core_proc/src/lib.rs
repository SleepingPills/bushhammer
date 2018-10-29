#![feature(proc_macro_diagnostic)]
#![feature(proc_macro_span)]
#![recursion_limit = "256"]
#![allow(unused_imports, dead_code, unused_variables, unused_mut)]

extern crate proc_macro;
extern crate proc_macro2;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use std::mem;

mod util;
mod sysdef;
mod parse;
mod build;


#[proc_macro_attribute]
pub fn make_system(attr: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut struct_body: syn::ItemStruct = syn::parse(item).unwrap();

    let sys_name = struct_body.ident.to_string();
    let sys_mod_name = sys_name.to_lowercase() + "_mod";

    // Construct the name of the actual data field
    let sys_data_type_name = sys_name.clone() + "Data";
    let sys_data_type_path = format!("{}::{}", sys_mod_name, sys_data_type_name);

    // Create the actual data type and swap it out with the placeholder
    let raw_sys_def = swap_sys_def(&mut struct_body, sys_data_type_path);

    let (provide_ent_id, comp_def) = parse::parse_raw_sys_def(raw_sys_def);

    let sys_def = sysdef::SystemDef::new(provide_ent_id, comp_def);

    // Construct the various type identifiers
    let mod_ident = parse::parse_string::<syn::Ident>(&(sys_name.to_lowercase() + "_mod"), "Error constructing system module ident:");
    let sys_data_ident = parse::parse_string::<syn::Ident>(&sys_data_type_name, "Error constructing system struct ident:");
    let sys_ctx_ident = parse::parse_string::<syn::Ident>(&(sys_name.clone() + "Context"), "Error constructing context ident:");
    let sys_iter_ident = parse::parse_string::<syn::Ident>(&(sys_name.clone() + "Iter"), "Error constructing iterator ident:");

    // Construct the support structs and infrastructure
    let sys_data_struct = build::create_sys_data_struct(&sys_data_ident, &sys_ctx_ident, &sys_def);
    let sys_ctx_struct = build::create_sys_ctx_struct(&sys_ctx_ident, &sys_iter_ident, &sys_def);
    let sys_iter_struct = build::create_sys_iter_struct(&sys_iter_ident, &sys_def);

    let result = quote! {
        pub mod #mod_ident {
            use indexmap::IndexMap;
            use indexmap::map;
            use t51core::component::{ComponentStore, ComponentField};
            use t51core::entity::EntityId;
            use t51core::sync::{RwGuard, ReadGuard};

            #sys_data_struct
            #sys_ctx_struct
            #sys_iter_struct
        }

        #struct_body
    };

    result.into()
}


fn swap_sys_def(struct_body: &mut syn::ItemStruct, sys_data_type_path: String) -> syn::TypePath {
    // Get the system data definition field by going through all fields and finding the first that has
    // the SystemData type.
    let data_field = struct_body
        .fields
        .iter_mut()
        .find(|field| match &field.ty {
            syn::Type::Path(path) => path.path.segments.iter().any(|seg| seg.ident == "SystemData"),
            _ => false,
        })
        .expect("System Data field missing");

    // Construct the AST for the actual data type
    let mut actual_type = parse::parse_string::<syn::Type>(sys_data_type_path.as_str(), "Failed constructing system data type {}");

    // Swap out the placeholder type with the actual one
    let placeholder_type = match mem::replace(&mut data_field.ty, actual_type) {
        syn::Type::Path(path) => path,
        _ => unreachable!(),
    };

    placeholder_type
}
