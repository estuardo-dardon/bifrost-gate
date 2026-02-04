use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType, Type, TypePath};

#[proc_macro_attribute]
pub fn auto_instrument(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let vis = &input.vis;
    let sig = &input.sig;
    let block = &input.block;
    let attrs = &input.attrs;
    let fn_name = sig.ident.to_string();

    // detect if function is async
    let is_async = sig.asyncness.is_some();

    // detect if return type is Result<..., ...>
    let mut returns_result = false;
    if let ReturnType::Type(_, ty_box) = &sig.output {
        if let Type::Path(TypePath { path, .. }) = &**ty_box {
            if let Some(seg) = path.segments.last() {
                if seg.ident == "Result" {
                    returns_result = true;
                }
            }
        }
    }

    let expanded = if is_async {
        if returns_result {
            quote! {
                #(#attrs)*
                #vis #sig {
                    crate::logger::auto_instrument_enter(#fn_name);
                    async move {
                        let __ai_res = (async move #block).await;
                        // log exit
                        crate::logger::auto_instrument_exit(#fn_name);
                        // if Err, log error
                        match &__ai_res {
                            Err(e) => crate::logger::auto_instrument_error(#fn_name, &format!("{:?}", e)),
                            _ => {}
                        }
                        __ai_res
                    }.await
                }
            }
        } else {
            quote! {
                #(#attrs)*
                #vis #sig {
                    crate::logger::auto_instrument_enter(#fn_name);
                    async move {
                        let __ai_res = (async move #block).await;
                        crate::logger::auto_instrument_exit(#fn_name);
                        __ai_res
                    }.await
                }
            }
        }
    } else {
        if returns_result {
            quote! {
                #(#attrs)*
                #vis #sig {
                    crate::logger::auto_instrument_enter(#fn_name);
                    let __ai_res = (|| #block)();
                    crate::logger::auto_instrument_exit(#fn_name);
                    match &__ai_res {
                        Err(e) => crate::logger::auto_instrument_error(#fn_name, &format!("{:?}", e)),
                        _ => {}
                    }
                    __ai_res
                }
            }
        } else {
            quote! {
                #(#attrs)*
                #vis #sig {
                    crate::logger::auto_instrument_enter(#fn_name);
                    let __ai_res = (|| #block)();
                    crate::logger::auto_instrument_exit(#fn_name);
                    __ai_res
                }
            }
        }
    };

    TokenStream::from(expanded)
}
