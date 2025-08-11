extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use stringcase::pascal_case;
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{Ident, Token, parse_macro_input};

struct Receivers {
    receivers: Punctuated<Ident, Token![,]>,
}

#[derive(Debug)]
struct Receiver {
    ident: Ident,
    value_ident: Ident,
    gen_type: Ident,
}

impl Parse for Receivers {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let receivers = input.parse_terminated(Ident::parse, Token![,])?;
        Ok(Receivers { receivers })
    }
}

#[proc_macro]
pub fn await_ready(input: TokenStream) -> TokenStream {
    let Receivers { receivers } = parse_macro_input!(input as Receivers);
    let receivers = receivers
        .iter()
        .map(|ident| {
            let ident_name = ident.to_string();
            let value_ident_name = ident_name.trim_end_matches("_rx");

            Receiver {
                ident: ident.clone(),
                value_ident: format_ident!("{}", value_ident_name),
                gen_type: format_ident!("T{}", pascal_case(value_ident_name)),
            }
        })
        .collect::<Vec<_>>();

    let awaited_gets = receivers
        .iter()
        .map(|r| &r.ident)
        .map(|r| quote!(self.#r.get().await))
        .collect::<Vec<_>>();
    let gen_types = receivers.iter().map(|r| &r.gen_type).collect::<Vec<_>>();
    let fields = receivers
        .iter()
        .map(|r| {
            let ident = &r.ident;
            let gen_type = &r.gen_type;
            quote!(#ident: Receiver<#gen_type>)
        })
        .collect::<Vec<_>>();
    let fields_init = receivers
        .iter()
        .map(|r| {
            let ident = &r.ident;
            quote!(#ident: #ident.clone())
        })
        .collect::<Vec<_>>();
    let fields_move_init = receivers
        .iter()
        .map(|r| {
            let ident = &r.ident;
            quote!(#ident: self.#ident)
        })
        .collect::<Vec<_>>();
    let matched_values = receivers
        .iter()
        .map(|r| {
            let value_ident = &r.value_ident;
            quote!(Some(#value_ident))
        })
        .collect::<Vec<_>>();
    let value_idents = receivers.iter().map(|r| &r.value_ident).collect::<Vec<_>>();
    let receivers_count = receivers.len();
    let not_ready_checks = value_idents
        .iter()
        .map(|value_ident| {
            quote!(if #value_ident.is_none() { not_ready.push(stringify!(#value_ident)); })
        })
        .collect::<Vec<_>>();

    let block = quote! {{
        use std::future::Future;
        use tracing::{debug, enabled, info, Level};
        use vg_core::sync::signal::Receiver;

        struct Configurator<#(#gen_types), *>
        where
            #(#gen_types: PartialEq + Clone), *
        {
            #(#fields),*
        }

        struct Runner<F, Fut, #(#gen_types), *>
        where
            F: FnOnce(#(#gen_types), *) -> Fut,
            Fut: Future<Output = ()>,
            #(#gen_types: PartialEq + Clone), *
        {
            and_then_fn: F,
            #(#fields),*
        }

        impl<#(#gen_types), *> Configurator<#(#gen_types), *>
        where
            #(#gen_types: PartialEq + Clone), *
        {
            #[must_use]
            pub fn and_then<F, Fut>(self, f: F) -> Runner<F, Fut, #(#gen_types),*>
            where
                F: FnOnce(#(#gen_types), *) -> Fut,
                Fut: Future<Output = ()>
            {
                Runner {
                    and_then_fn: f,
                    #(#fields_move_init),*
                }
            }
        }

        impl<F, Fut, #(#gen_types), *> Runner<F, Fut, #(#gen_types), *>
        where
            F: FnOnce(#(#gen_types), *) -> Fut,
            Fut: Future<Output = ()>,
            #(#gen_types: PartialEq + Clone), *
        {
            pub async fn run(self) {
                match (#(#awaited_gets), *) {
                    (#(#matched_values), *) => {
                        (self.and_then_fn)(#(#value_idents),*).await
                    }
                    (#(#value_idents), *) => {
                        if enabled!(Level::DEBUG) {
                            let mut not_ready: Vec<&str> = Vec::with_capacity(#receivers_count);
                            #(#not_ready_checks)*
                            let not_ready_str = not_ready.join(", ");
                            debug!("Awaited values not ready: {}", not_ready_str);
                        } else {
                            info!("Awaited values not ready");
                        }
                    }
                }
            }
        }

        Configurator {
            #(#fields_init),*
        }
    }};

    block.into()
}
