extern crate proc_macro;

use syn;

#[proc_macro_derive(Topic)]
pub fn derive_topic(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    derive_core(
        &ast.ident.to_string(),
        "Topic",
        "TopicId",
        "acquire_topic_id",
        "get_topic_id",
    )
}

#[proc_macro_derive(Component)]
pub fn derive_component(item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(item).unwrap();
    derive_core(
        &ast.ident.to_string(),
        "Component",
        "ComponentId",
        "acquire_unique_id",
        "get_unique_id",
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
    let static_once = format!("__{}_INIT", struct_name.to_uppercase());

    let tokens = format!(
        r###"

        mod {static_mod} {{
            use super::{id_type};
            use std::sync;

            pub(crate) static mut {static_id}: {id_type} = {id_type}{{id: 0}};
            pub(crate) static {static_once}: sync::Once = sync::Once::new();
        }}

        impl {main_trait} for {struct_name} {{
            #[inline]
            fn {acquire_name}() -> {id_type} {{
                unsafe {{
                    {static_mod}::{static_once}.call_once(|| {{
                        let counter = {id_type}::get_name_vec().len();
                        {static_mod}::{static_id} = {id_type}::new::<{struct_name}>(counter);

                        {id_type}::get_name_vec().push("{struct_name}");
                        {id_type}::get_id_vec().push({static_mod}::{static_id});
                    }});

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
        static_once = static_once,
        main_trait = main_trait,
        struct_name = struct_name,
        acquire_name = acquire_name,
        getter_name = getter_name
    );

    tokens.parse().unwrap()
}
