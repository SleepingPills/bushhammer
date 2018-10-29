use crate::sysdef::SystemDef;
use crate::util;
use proc_macro2;
use syn;

pub(crate) fn create_sys_data_struct(
    sys_data_ident: &syn::Ident,
    sys_ctx_ident: &syn::Ident,
    sys_def: &SystemDef,
) -> proc_macro2::TokenStream {
    let idx_map = create_indexmap_type(sys_def.comp_ident.len());

    let comp_ident = &sys_def.comp_ident;
    let comp_types = &sys_def.comp_types;

    // Construct the guard identifiers
    let guards: Vec<_> = (0..sys_def.comp_ident.len()).map(util::guard_ident).collect();

    // Construct the guard declarations
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

    // Construct the guard value assignment for creating a context
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

    // Construct the system data implementation
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

pub(crate) fn create_sys_ctx_struct(
    sys_ctx_ident: &syn::Ident,
    sys_iter_ident: &syn::Ident,
    sys_def: &SystemDef,
) -> proc_macro2::TokenStream {
    let idx_map = create_indexmap_type(sys_def.comp_ident.len());

    // Construct the guard declarations
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

    // Construct the indexers for the getter method
    let indexers: Vec<_> = sys_def
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
    let ptr_fields = &sys_def.ptr_fields;

    // Construct the iterator return tuple
    let iter_tuple = match sys_def.provide_ent_id {
        true => quote!((EntityId, #(&'a #comp_types_mut),*)),
        _ => quote!((#(&'a #comp_types_mut),*)),
    };

    // Construct the context implementation
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
                unsafe { (#(#indexers),*) }
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

pub(crate) fn create_sys_iter_struct(sys_iter_ident: &syn::Ident, sys_def: &SystemDef) -> proc_macro2::TokenStream {
    let comp_ident = &sys_def.comp_ident;
    let comp_ident_dup = &sys_def.comp_ident;
    let comp_types_mut = &sys_def.comp_types_mut;
    let ptr_fields = &sys_def.ptr_fields;

    // Construct the map iterator type
    let usize_vec = create_usize_tuple(sys_def.comp_ident.len());
    let map_iter = quote!{map::Iter<'a, usize, (#(#usize_vec),*)>};

    // Construct the iterator return tuple type
    let iter_tuple = match sys_def.provide_ent_id {
        true => quote!((EntityId, #(&'a #comp_types_mut),*)),
        _ => quote!((#(&'a #comp_types_mut),*)),
    };

    // Construct the indexer logic for grabbing items from the pointers
    let mut indexers = Vec::new();

    if sys_def.provide_ent_id {
        indexers.push(quote!(*id));
    }

    for (i, ident) in sys_def.comp_ident.iter().enumerate() {
        let idx = match &sys_def.mutability[i] {
            true => quote!(&mut *self.#ident.add(#ident)),
            _ => quote!(&*self.#ident.add(#ident)),
        };

        indexers.push(idx);
    }

    // Construct the iterator implementation
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
