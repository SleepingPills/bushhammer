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
use syn::spanned::Spanned;
use syn::token::Token;
use syn::visit::Visit;

#[derive(Debug)]
struct SystemDef {
    provide_ent_id: bool,
    mutability: Vec<bool>,
    comp_ident: Vec<syn::Ident>,
    comp_types: Vec<syn::TypePath>,
    comp_types_mut: Vec<proc_macro2::TokenStream>,
    ptr_fields: proc_macro2::TokenStream,
    iter_tup: proc_macro2::TokenStream,
}

fn create_sys_def(provide_ent_id: bool, comp_def: Vec<(bool, syn::TypePath)>) -> SystemDef {
    let mut mutability = Vec::new();
    let mut comp_ident = Vec::new();
    let mut comp_types = Vec::new();
    let mut comp_types_mut = Vec::new();
    let mut ptr_field_tokens = Vec::new();

    for (idx, (mutable, ty)) in comp_def.iter().enumerate() {
        mutability.push(*mutable);
        comp_ident.push(create_comp_ident(idx));
        comp_types.push(ty.clone());

        let (ty_mut, ptr_token) = match mutable {
            true => (quote!(mut #ty), quote!(*mut #ty)),
            _ => (quote!(#ty), quote!(*const #ty)),
        };
        comp_types_mut.push(ty_mut);
        ptr_field_tokens.push(ptr_token)
    }

    let comp_ident_ref = &comp_ident;
    let comp_types_mut_ref = &comp_types_mut;
    let ptr_fields = quote!{#(#comp_ident_ref: #ptr_field_tokens),*};
    let iter_tup = quote!{(#(&'a #comp_types_mut_ref),*)};

    SystemDef {
        provide_ent_id,
        mutability,
        comp_ident,
        comp_types,
        comp_types_mut,
        ptr_fields,
        iter_tup,
    }
}

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

    let (provide_ent_id, comp_def) = match parse_sys_def(raw_sys_def) {
        Some(results) => results,
        _ => unreachable!(),
    };

    let sys_def = create_sys_def(provide_ent_id, comp_def);

    let mod_ident = parse_string::<syn::Ident>(sys_mod_name.as_str(), "Error constructing system module:");
    let sys_data_ident = parse_string::<syn::Ident>(sys_data_type_name.as_str(), "Error constructing system struct ident:");
    let sys_ctx_ident = parse_string::<syn::Ident>((sys_name.clone() + "Context").as_str(), "Error constructing context ident:");
    let sys_iter_ident = parse_string::<syn::Ident>((sys_name.clone() + "Iter").as_str(), "Error constructing iterator ident:");

    let sys_data_struct = create_sys_data_struct(&sys_data_ident, &sys_ctx_ident, &sys_def);
    let sys_ctx_struct = create_sys_ctx_struct(&sys_ctx_ident, &sys_iter_ident, &sys_def);
    let sys_iter_struct = create_sys_iter_struct(&sys_iter_ident, &sys_def);

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

fn create_sys_iter_struct(sys_iter_ident: &syn::Ident, sys_def: &SystemDef) -> proc_macro2::TokenStream {
    let comp_ident = &sys_def.comp_ident;
    let comp_ident_dup = &sys_def.comp_ident;
    let comp_types_mut = &sys_def.comp_types_mut;

    let ptr_fields = &sys_def.ptr_fields;

    let usize_vec = create_usize_tuple(sys_def.comp_ident.len());
    let map_iter = quote!{map::Iter<'a, usize, (#(#usize_vec),*)>};

    let iter_tuple = match sys_def.provide_ent_id {
        true => quote!((EntityId, #(&'a #comp_types_mut),*)),
        _ => quote!((#(&'a #comp_types_mut),*)),
    };

    let mut indexers = Vec::new();

    if sys_def.provide_ent_id {
        indexers.push(quote!(*id));
    }

    for (i, ident) in sys_def.comp_ident.iter().enumerate() {
        let idx = match &sys_def.mutability[i] {
            true => quote!(&mut *self.#ident.add(#ident)),
            _ => quote!(&*self.#ident.add(#ident))
        };

        indexers.push(idx);
    }

    quote!{
        pub struct #sys_iter_ident<'a> {
            entity_iter: #map_iter,
            #ptr_fields
        }

        impl<'a> Iterator for #sys_iter_ident<'a> {
            type Item = #iter_tuple;

            #[inline(always)]
            fn next(&mut self) -> Option<#iter_tuple> {
                match self.entity_iter.next() {
                    Some((id, &(#(#comp_ident),*))) => Some(unsafe { (#(#indexers),*) }),
                    _ => None,
                }
            }
        }
    }
}

fn create_sys_ctx_struct(
    sys_ctx_ident: &syn::Ident,
    sys_iter_ident: &syn::Ident,
    sys_def: &SystemDef,
) -> proc_macro2::TokenStream {
    let idx_map = create_indexmap_type(sys_def.comp_ident.len());

    let guard_decl: Vec<_> = sys_def
        .comp_types
        .iter()
        .enumerate()
        .map(|(idx, ty)| {
            let mutable = sys_def.mutability[idx];
            match mutable {
                true => quote!(RwGuard<ComponentStore<u64>>),
                _ => quote!(ReadGuard<ComponentStore<#ty>>),
            }
        })
        .collect();

    let get_return: Vec<_> = sys_def
        .comp_ident
        .iter()
        .enumerate()
        .map(|(idx, ident)| {
            let mutable = sys_def.mutability[idx];
            match mutable {
                true => quote!(&mut *self.#ident.add(#ident)),
                _ => quote!(&*self.#ident.add(#ident)),
            }
        })
        .collect();

    let comp_ident = &sys_def.comp_ident;
    let comp_ident_dup = &sys_def.comp_ident;
    let comp_types_mut = &sys_def.comp_types_mut;
    let iter_tuple = match sys_def.provide_ent_id {
        true => quote!((EntityId, #(&'a #comp_types_mut),*)),
        _ => quote!((#(&'a #comp_types_mut),*)),
    };

    let ptr_fields = &sys_def.ptr_fields;
    quote!{
        pub struct #sys_ctx_ident<'a> {
            entities: &'a #idx_map,
            #ptr_fields,
            _guards: (#(#guard_decl),*)
        }

        impl<'a> #sys_ctx_ident<'a> {
            #[inline(always)]
            pub fn iter(&self) -> #sys_iter_ident {
                #sys_iter_ident {
                    entity_iter: self.entities.iter(),
                    #(#comp_ident: self.#comp_ident_dup),*
                }
            }

            #[inline(always)]
            pub unsafe fn get_by_id(&self, id: usize) -> (#(&#comp_types_mut),*) {
                let (#(#comp_ident),*) = self.entities[&id];
                unsafe { (#(#get_return),*) }
            }
        }

        impl<'a> IntoIterator for #sys_ctx_ident<'a> {
            type Item = #iter_tuple;
            type IntoIter = #sys_iter_ident<'a>;

            #[inline(always)]
            fn into_iter(self) -> #sys_iter_ident<'a> {
                #sys_iter_ident {
                    entity_iter: self.entities.iter(),
                    #(#comp_ident: self.#comp_ident_dup),*
                }
            }
        }
    }
}

fn create_sys_data_struct(
    sys_data_ident: &syn::Ident,
    sys_ctx_ident: &syn::Ident,
    sys_def: &SystemDef,
) -> proc_macro2::TokenStream {
    let idx_map = create_indexmap_type(sys_def.comp_ident.len());

    let comp_ident = &sys_def.comp_ident;
    let comp_types = &sys_def.comp_types;

    let guards: Vec<_> = (0..sys_def.comp_ident.len()).map(create_guard_ident).collect();

    let guard_decl: Vec<_> = comp_ident
        .iter()
        .enumerate()
        .map(|(idx, ident)| {
            let mutable = sys_def.mutability[idx];
            let guard = &guards[idx];
            match mutable {
                true => quote!(let mut #guard = self.#ident.write()),
                _ => quote!(let #guard = self.#ident.read()),
            }
        })
        .collect();

    let guard_assign: Vec<_> = comp_ident
        .iter()
        .enumerate()
        .map(|(idx, ident)| {
            let mutable = sys_def.mutability[idx];
            let guard = &guards[idx];
            match mutable {
                true => quote!(#ident: #guard.get_pool_mut_ptr()),
                _ => quote!(#ident: #guard.get_pool_ptr()),
            }
        })
        .collect();

    let guards_ref = &guards;
    quote!{
        pub struct #sys_data_ident {
            entities: #idx_map,
            #(#comp_ident: ComponentField<#comp_types>),*
        }

        impl #sys_data_ident {
            #[inline]
            pub fn get_ctx(&self) -> #sys_ctx_ident {
                #(#guard_decl);*;

                unsafe {
                    MySysContext {
                        entities: &self.entities,
                        #(#guard_assign),*,
                        _guards: (#(#guards_ref),*)
                    }
                }
            }
        }
    }
}

fn create_indexmap_type(rank: usize) -> proc_macro2::TokenStream {
    let usize_vec = create_usize_tuple(rank);
    quote!{IndexMap<usize, (#(#usize_vec),*)>}
}

fn create_usize_tuple(rank: usize) -> Vec<proc_macro2::TokenStream> {
    let id_type = quote!(usize);
    let mut usize_vec = Vec::new();
    for _ in 0..rank {
        usize_vec.push(id_type.clone());
    }
    usize_vec
}

fn create_comp_ident(counter: usize) -> syn::Ident {
    syn::Ident::new(format!("comp_{}", counter).as_str(), proc_macro2::Span::call_site())
}

fn create_guard_ident(counter: usize) -> syn::Ident {
    syn::Ident::new(format!("guard_{}", counter).as_str(), proc_macro2::Span::call_site())
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

/// Checks whether the entity id is requested to be in the component iterator
fn check_entity_id(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|seg| seg.ident == "EntityId"),
        _ => false,
    }
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
    let mut actual_type = parse_string::<syn::Type>(sys_data_type_path.as_str(), "Failed constructing system data type {}");

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
