use syn;
use syn::spanned::Spanned;

pub(crate) fn parse_raw_sys_def(raw_sys_def: syn::TypePath) -> (bool, Vec<(bool, syn::TypePath)>) {
    let result = raw_sys_def.path.segments.iter().find_map(|seg| {
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
    });

    // At this point, the system definition is certain to exist, so just unpack it
    match result {
        Some(results) => results,
        _ => unreachable!(),
    }
}

/// Parse the contents of the system definition tuple, returning a flag whether to include the entity id
/// in the iteration logic and a vector of (mutability, type) tuples for each component.
pub(crate) fn parse_sys_def_tuple(elems: &syn::punctuated::Punctuated<syn::Type, Token![,]>) -> (bool, Vec<(bool, syn::TypePath)>) {
    let provide_ent_id = match elems.first() {
        Some(syn::punctuated::Pair::Punctuated(value, _)) => check_entity_id(&value),
        _ => fail_parse(elems.span(), "System must specify at least one component"),
    };

    match provide_ent_id {
        true => (true, elems.iter().skip(1).map(parse_sys_def_tuple_entries).collect()),
        _ => (false, elems.iter().map(parse_sys_def_tuple_entries).collect()),
    }
}

/// Extract a tuple of (mutability, type) for each entry in the system definition tuple
pub(crate) fn parse_sys_def_tuple_entries(elem: &syn::Type) -> (bool, syn::TypePath) {
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
pub(crate) fn check_entity_id(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path.path.segments.iter().any(|seg| seg.ident == "EntityId"),
        _ => false,
    }
}

/// Parse a string into the requested syntax, or panic if it fails
pub(crate) fn parse_string<T: syn::parse::Parse>(string: &str, error_msg: &str) -> T {
    match syn::parse_str::<T>(string) {
        Ok(result) => result,
        Err(error) => panic!(error_msg.to_owned() + " " + &error.to_string()),
    }
}

/// Fail the parsin and emit a span specific and user friendly error message
pub(crate) fn fail_parse(span: proc_macro2::Span, msg: &str) -> ! {
    span.unstable().error(msg).emit();
    panic!("Incorrect system definition");
}
