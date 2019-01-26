extern crate proc_macro;

use syn;

#[proc_macro_derive(Message)]
pub fn derive_message(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    derive_core(
        &ast.ident.to_string(),
        "Message",
        "Topic",
        "acquire_topic_id",
        "get_topic",
    )
}

fn derive_core(
    struct_name: &str,
    main_trait: &str,
    id_type: &str,
    acquire_name: &str,
    getter_name: &str,
) -> proc_macro::TokenStream {
    let static_mod = format!("__{}Module", struct_name.to_uppercase());
    let static_id = format!("__{}_ID", struct_name.to_uppercase());

    let tokens = format!(
        r###"

        mod {static_mod} {{
            use super::{id_type};

            pub(crate) static mut {static_id}: {id_type} = {id_type}{{id: 0}};
        }}

        impl {main_trait} for {struct_name} {{
            #[inline]
            fn {acquire_name}() -> {id_type} {{
                unsafe {{
                    let counter = {id_type}::get_name_vec().len();
                    {static_mod}::{static_id} = {id_type}::new::<{struct_name}>(counter);

                    {id_type}::get_name_vec().push("{struct_name}");
                    {id_type}::get_id_vec().push({static_mod}::{static_id});

                    {static_mod}::{static_id}
                }}
            }}

            #[inline]
            fn {getter_name}() -> {id_type} {{
                unsafe {{
                    {static_mod}::{static_id}
                }}
            }}
        }}"###,
        static_mod = static_mod,
        static_id = static_id,
        id_type = id_type,
        main_trait = main_trait,
        struct_name = struct_name,
        acquire_name = acquire_name,
        getter_name = getter_name
    );

    tokens.parse().unwrap()
}
