use syn;
use proc_macro2;


pub(crate) fn comp_ident(counter: usize) -> syn::Ident {
    syn::Ident::new(format!("comp_{}", counter).as_str(), proc_macro2::Span::call_site())
}

pub(crate) fn guard_ident(counter: usize) -> syn::Ident {
    syn::Ident::new(format!("guard_{}", counter).as_str(), proc_macro2::Span::call_site())
}
