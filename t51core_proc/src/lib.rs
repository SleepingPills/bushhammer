#![feature(proc_macro_diagnostic)]
#![feature(proc_macro_span)]
#![recursion_limit = "256"]
#![allow(unused_imports, dead_code, unused_variables, unused_mut)]

extern crate proc_macro;
#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

use crate::proc_macro::TokenStream;
use std::mem;
use syn::spanned::Spanned;
use syn::token::Token;
use syn::visit::Visit;

/*
SystemDataFold will run a fold op through the AST. When it encounters the SystemData field, it replaces
it with the appropriate thing, and then stashes away the data. It then continues parsing and if it encounters
another one, it will fail.
*/

#[proc_macro_attribute]
pub fn make_system2(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut struct_body: syn::ItemStruct = syn::parse(item).unwrap();

    let system_name = struct_body.ident.to_string();
    let module_name = system_name.to_lowercase() + "_mod";

    // Construct the name of the actual data field
    let actual_type_name = system_name + "Data";
    let actual_type_path = format!("{}::{}", module_name, actual_type_name);

    // Create the actual data type and swap it out with the placeholder
    let sys_def = create_and_swap_data_field(&mut struct_body, &actual_type_path);

    let (provide_ent_id, sys_def) = match parse_sys_def(sys_def) {
        Some(results) => results,
        _ => unreachable!(),
    };

    println!("{:#?}", sys_def);

    let mod_ident = parse_string::<syn::Ident>(module_name.as_str(), "Error constructing system module: {}");
    let actual_type_ident = parse_string::<syn::Ident>(actual_type_name.as_str(), "Error constructing system struct ident: {}");

    let result = quote! {
        pub mod #mod_ident {
            pub struct #actual_type_ident {

            }
        }

        #struct_body
    };

    result.into()
}

fn parse_sys_def(sys_def: syn::TypePath) -> Option<(bool, Vec<(bool, syn::TypePath)>)> {
    sys_def.path.segments.iter().find_map(|seg| {
        if seg.ident == "SystemData" {
            match &seg.arguments {
                syn::PathArguments::AngleBracketed(generic_args) => {
                    let args = &generic_args.args;

                    if args.len() != 1 {
                        fail_parse(args.span(), "SystemData definition must contain exactly one type argument");
                    }

                    match &args[0] {
                        syn::GenericArgument::Type(syn::Type::Tuple(tup)) => Some(parse_sys_def_tuple(&tup.elems)),
                        _ => fail_parse(args.span(), "SystemData definition must contain a tuple"),
                    }
                }
                _ => fail_parse(seg.arguments.span(), "Malformed SystemData definition"),
            }
        } else {
            None
        }
    })
}

fn parse_sys_def_tuple(elems: &syn::punctuated::Punctuated<syn::Type, Token![,]>) -> (bool, Vec<(bool, syn::TypePath)>) {
    let provide_ent_id = match elems.first() {
        Some(syn::punctuated::Pair::Punctuated(value, _)) => check_entity_id(&value),
        _ => fail_parse(elems.span(), "System must specify at least one component"),
    };

    match provide_ent_id {
        true => (true, elems.iter().skip(1).map(parse_sys_def_tuple_entries).collect()),
        _ => (false, elems.iter().map(parse_sys_def_tuple_entries).collect()),
    }
}

fn parse_sys_def_tuple_entries(elem: &syn::Type) -> (bool, syn::TypePath) {
    if let syn::Type::Reference(ref_elem) = elem {
        if let syn::Type::Path(path) = &*ref_elem.elem {
            return (ref_elem.mutability.is_some(), path.clone());
        } else {
            fail_parse(ref_elem.span(), "SystemData tuple must contain non-generic struct types")
        }
    } else {
        fail_parse(elem.span(), "Must be a reference to a component")
    }
}

fn check_entity_id(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|seg| seg.ident == "EntityId"),
        _ => false,
    }
}

fn create_and_swap_data_field(struct_body: &mut syn::ItemStruct, actual_type_path: &String) -> syn::TypePath {
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
    let mut actual_type = parse_string::<syn::Type>(actual_type_path.as_str(), "Failed constructing system data type {}");

    // Swap out the placeholder type with the actual one
    let placeholder_type = match mem::replace(&mut data_field.ty, actual_type) {
        syn::Type::Path(path) => path,
        _ => unreachable!(),
    };

    placeholder_type
}

fn parse_string<T: syn::parse::Parse>(string: &str, error_msg: &str) -> T {
    match syn::parse_str::<T>(string) {
        Ok(result) => result,
        Err(error) => panic!(error_msg.to_owned() + " " + &error.to_string()),
    }
}

fn fail_parse(span: proc_macro2::Span, msg: &str) -> ! {
    span.unstable().error(msg).emit();
    panic!("Incorrect system definition");
}

#[proc_macro_attribute]
pub fn make_system(attr: TokenStream, item: TokenStream) -> TokenStream {
    let gen = quote! {
        pub mod mysys_mod {
            use indexmap::IndexMap;
            use indexmap::map;
            use t51core::component::{ComponentStore, ComponentField};
            use t51core::sync::{RwGuard, ReadGuard};

            pub struct MySysData {
                entities: IndexMap<usize, (usize, usize, usize)>,
                comp_a: ComponentField<i32>,
                comp_b: ComponentField<u64>,
                comp_c: ComponentField<u64>,
            }

            impl MySysData {
                pub fn get_ctx(&self) -> MySysContext {
                    let comp_a_guard = self.comp_a.read();
                    let comp_b_guard = self.comp_b.read();
                    let mut comp_c_guard = self.comp_c.write();

                    unsafe {
                        MySysContext {
                            entities: &self.entities,
                            comp_a: comp_a_guard.get_pool_ptr(),
                            comp_b: comp_b_guard.get_pool_ptr(),
                            comp_c: comp_c_guard.get_pool_mut_ptr(),
                            _guards: (comp_a_guard, comp_b_guard, comp_c_guard),
                        }
                    }
                }
            }

            pub struct MySysContext<'a> {
                entities: &'a IndexMap<usize, (usize, usize, usize)>,
                comp_a: *const i32,
                comp_b: *const u64,
                comp_c: *mut u64,
                _guards: (
                    ReadGuard<ComponentStore<i32>>,
                    ReadGuard<ComponentStore<u64>>,
                    RwGuard<ComponentStore<u64>>,
                ),
            }

            impl<'a> MySysContext<'a> {
                pub fn iter(&self) -> MySysDataIter {
                    MySysDataIter {
                        entity_iter: self.entities.iter(),
                        comp_a: self.comp_a,
                        comp_b: self.comp_b,
                        comp_c: self.comp_c,
                    }
                }

                #[inline(always)]
                pub unsafe fn get_by_id(&self, id: usize) -> (&i32, &u64, &mut u64) {
                    let (a_idx, b_idx, c_idx) = self.entities[&id];
                    unsafe { (&*self.comp_a.add(a_idx), &*self.comp_b.add(b_idx), &mut *self.comp_c.add(c_idx)) }
                }
            }

            impl<'a> IntoIterator for MySysContext<'a> {
                type Item = (&'a i32, &'a u64, &'a mut u64);
                type IntoIter = MySysDataIter<'a>;

                fn into_iter(self) -> MySysDataIter<'a> {
                    MySysDataIter {
                        entity_iter: self.entities.iter(),
                        comp_a: self.comp_a,
                        comp_b: self.comp_b,
                        comp_c: self.comp_c,
                    }
                }
            }

            pub struct MySysDataIter<'a> {
                entity_iter: map::Iter<'a, usize, (usize, usize, usize)>,
                comp_a: *const i32,
                comp_b: *const u64,
                comp_c: *mut u64,
            }

            impl<'a> Iterator for MySysDataIter<'a> {
                type Item = (&'a i32, &'a u64, &'a mut u64);

                fn next(&mut self) -> Option<(&'a i32, &'a u64, &'a mut u64)> {
                    match self.entity_iter.next() {
                        Some((&id, &(a, b, c))) => {
                            Some(unsafe {(
                                &*self.comp_a.add(a),
                                &*self.comp_b.add(b),
                                &mut *self.comp_c.add(c)
                            )})
                        },
                        _ => None,
                    }
                }
            }
        }

        pub struct MySys {
            data: mysys_mod::MySysData,
        }
    };
    gen.into()
}
