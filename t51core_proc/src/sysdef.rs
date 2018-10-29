use proc_macro2;
use crate::util;


#[derive(Debug)]
pub(crate) struct SystemDef {
    pub(crate) provide_ent_id: bool,
    pub(crate) mutability: Vec<bool>,
    pub(crate) comp_ident: Vec<syn::Ident>,
    pub(crate) comp_types: Vec<syn::TypePath>,
    pub(crate) comp_types_mut: Vec<proc_macro2::TokenStream>,
    pub(crate) ptr_fields: proc_macro2::TokenStream,
}

impl SystemDef {
    pub(crate) fn new(provide_ent_id: bool, comp_def: Vec<(bool, syn::TypePath)>) -> SystemDef {
        let mut mutability = Vec::new();
        let mut comp_ident = Vec::new();
        let mut comp_types = Vec::new();
        let mut comp_types_mut = Vec::new();
        let mut ptr_field_tokens = Vec::new();

        for (idx, (mutable, ty)) in comp_def.iter().enumerate() {
            mutability.push(*mutable);
            comp_ident.push(util::comp_ident(idx));
            comp_types.push(ty.clone());

            let (ty_mut, ptr_token) = match mutable {
                true => (quote!(mut #ty), quote!(*mut #ty)),
                _ => (quote!(#ty), quote!(*const #ty)),
            };
            comp_types_mut.push(ty_mut);
            ptr_field_tokens.push(ptr_token)
        }

        let comp_ident_ref = &comp_ident;
        let ptr_fields = quote!{#(#comp_ident_ref: #ptr_field_tokens),*};

        SystemDef {
            provide_ent_id,
            mutability,
            comp_ident,
            comp_types,
            comp_types_mut,
            ptr_fields,
        }
    }
}
