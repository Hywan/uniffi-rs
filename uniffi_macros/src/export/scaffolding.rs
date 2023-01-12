/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{spanned::Spanned, FnArg, Pat};

use super::{AsyncRuntime, ExportAttributeArguments, FunctionReturn, Signature};

pub(super) fn gen_fn_scaffolding(
    sig: &Signature,
    mod_path: &[String],
    checksum: u16,
    arguments: &ExportAttributeArguments,
) -> TokenStream {
    let name = &sig.ident;
    let name_s = name.to_string();

    let ffi_ident = Ident::new(
        &uniffi_meta::fn_ffi_symbol_name(mod_path, &name_s, checksum),
        Span::call_site(),
    );

    const ERROR_MSG: &str =
        "uniffi::export must be used on the impl block, not its containing fn's";
    let (params, args): (Vec<_>, Vec<_>) = collect_params(&sig.inputs, ERROR_MSG).unzip();

    let fn_call = quote! {
        #name(#(#args),*)
    };

    gen_ffi_function(sig, ffi_ident, &params, fn_call, arguments)
}

pub(super) fn gen_method_scaffolding(
    sig: &Signature,
    mod_path: &[String],
    checksum: u16,
    self_ident: &Ident,
    arguments: &ExportAttributeArguments,
) -> TokenStream {
    let name = &sig.ident;
    let name_s = name.to_string();

    let ffi_name = format!("impl_{self_ident}_{name_s}");
    let ffi_ident = Ident::new(
        &uniffi_meta::fn_ffi_symbol_name(mod_path, &ffi_name, checksum),
        Span::call_site(),
    );

    let mut params_args = (Vec::new(), Vec::new());

    const RECEIVER_ERROR: &str = "unreachable: only first parameter can be method receiver";
    let mut assoc_fn_error = None;
    let fn_call_prefix = match sig.inputs.first() {
        Some(arg) if is_receiver(arg) => {
            let ffi_converter = quote! {
                <::std::sync::Arc<#self_ident> as ::uniffi::FfiConverter>
            };

            params_args.0.push(quote! { this: #ffi_converter::FfiType });

            let remaining_args = sig.inputs.iter().skip(1);
            params_args.extend(collect_params(remaining_args, RECEIVER_ERROR));

            quote! {
                #ffi_converter::try_lift(this).unwrap_or_else(|err| {
                    ::std::panic!("Failed to convert arg 'self': {}", err)
                }).
            }
        }
        _ => {
            assoc_fn_error = Some(
                syn::Error::new_spanned(
                    &sig.ident,
                    "associated functions are not currently supported",
                )
                .into_compile_error(),
            );
            params_args.extend(collect_params(&sig.inputs, RECEIVER_ERROR));
            quote! { #self_ident:: }
        }
    };

    let (params, args) = params_args;

    let fn_call = quote! {
        #assoc_fn_error
        #fn_call_prefix #name(#(#args),*)
    };

    gen_ffi_function(sig, ffi_ident, &params, fn_call, arguments)
}

fn is_receiver(fn_arg: &FnArg) -> bool {
    match fn_arg {
        FnArg::Receiver(_) => true,
        FnArg::Typed(pat_ty) => matches!(&*pat_ty.pat, Pat::Ident(i) if i.ident == "self"),
    }
}

fn collect_params<'a>(
    inputs: impl IntoIterator<Item = &'a FnArg> + 'a,
    receiver_error_msg: &'static str,
) -> impl Iterator<Item = (TokenStream, TokenStream)> + 'a {
    fn receiver_error(
        receiver: impl ToTokens,
        receiver_error_msg: &str,
    ) -> (TokenStream, TokenStream) {
        let param = quote! { &self };
        let arg = syn::Error::new_spanned(receiver, receiver_error_msg).into_compile_error();
        (param, arg)
    }

    inputs.into_iter().enumerate().map(|(i, arg)| {
        let (ty, name) = match arg {
            FnArg::Receiver(r) => {
                return receiver_error(r, receiver_error_msg);
            }
            FnArg::Typed(pat_ty) => {
                let name = match &*pat_ty.pat {
                    Pat::Ident(i) if i.ident == "self" => {
                        return receiver_error(i, receiver_error_msg);
                    }
                    Pat::Ident(i) => Some(i.ident.to_string()),
                    _ => None,
                };

                (&pat_ty.ty, name)
            }
        };

        let arg_n = format_ident!("arg{i}");
        let param = quote! { #arg_n: <#ty as ::uniffi::FfiConverter>::FfiType };

        // FIXME: With UDL, fallible functions use uniffi::lower_anyhow_error_or_panic instead of
        // panicking unconditionally. This seems cleaner though.
        let panic_fmt = match name {
            Some(name) => format!("Failed to convert arg '{name}': {{}}"),
            None => format!("Failed to convert arg #{i}: {{}}"),
        };
        let arg = quote! {
            <#ty as ::uniffi::FfiConverter>::try_lift(#arg_n).unwrap_or_else(|err| {
                ::std::panic!(#panic_fmt, err)
            })
        };

        (param, arg)
    })
}

fn gen_ffi_function(
    sig: &Signature,
    ffi_ident: Ident,
    params: &[TokenStream],
    rust_fn_call: TokenStream,
    arguments: &ExportAttributeArguments,
) -> TokenStream {
    let name = sig.ident.to_string();
    let mut extra_functions = Vec::new();
    let is_async = sig.is_async;

    let (return_ty, throw_ty, return_expr, throws) = match &sig.output {
        Some(FunctionReturn { ty, throws: None }) if is_async => {
            let return_ty = quote! { #ty };
            let throw_ty = Some(quote! { ::std::convert::Infallible });

            (
                return_ty.clone(),
                throw_ty.clone(),
                quote! { Option<Box<::uniffi::RustFuture<#return_ty, #throw_ty>>> },
                &None,
            )
        }

        Some(FunctionReturn { ty, throws }) if is_async => {
            let return_ty = quote! { #ty };
            let throw_ty = Some(quote! { #throws });

            (
                return_ty.clone(),
                throw_ty.clone(),
                quote! { Option<Box<::uniffi::RustFuture<#return_ty, #throw_ty>>> },
                throws,
            )
        }

        None if is_async => {
            let return_ty = quote! { () };
            let throw_ty = Some(quote! { ::std::convert::Infallible });

            (
                return_ty.clone(),
                throw_ty.clone(),
                quote! { Option<Box<::uniffi::RustFuture<#return_ty, #throw_ty>>> },
                &None,
            )
        }

        Some(FunctionReturn { ty, throws }) => (
            quote! { #ty },
            None,
            quote! { <#ty as ::uniffi::FfiReturn>::FfiType },
            throws,
        ),

        None => (
            quote! { () },
            None,
            quote! { <() as ::uniffi::FfiReturn>::FfiType },
            &None,
        ),
    };

    let body_expr = if is_async {
        let rust_future_ctor = match &arguments.async_runtime {
            Some(AsyncRuntime::Tokio(_)) => quote! { new_tokio },
            None => quote! { new },
        };

        let body = match throws {
            Some(_) => quote! { #rust_fn_call.await },
            None => quote! { Ok(#rust_fn_call.await) },
        };

        quote! {
            ::uniffi::call_with_output(call_status, || {
                Some(Box::new(::uniffi::RustFuture::#rust_future_ctor(
                    async move {
                        #body
                    }
                )))
            })
        }
    } else {
        match throws {
            Some(error_ident) => {
                quote! {
                    ::uniffi::call_with_result(call_status, || {
                        let val = #rust_fn_call.map_err(|e| {
                            <#error_ident as ::uniffi::FfiConverter>::lower(
                                ::std::convert::Into::into(e),
                            )
                        })?;

                        Ok(<#return_ty as ::uniffi::FfiReturn>::lower(val))
                    })
                }
            }

            None => {
                quote! {
                    ::uniffi::call_with_output(call_status, || {
                        <#return_ty as ::uniffi::FfiReturn>::lower(#rust_fn_call)
                    })
                }
            }
        }
    };

    if is_async {
        let ffi_poll_ident = format_ident!("{}_poll", ffi_ident);
        let ffi_drop_ident = format_ident!("{}_drop", ffi_ident);

        // Monomorphised poll function.
        extra_functions.push(quote! {
            #[doc(hidden)]
            #[no_mangle]
            pub extern "C" fn #ffi_poll_ident(
                future: ::std::option::Option<&mut ::uniffi::RustFuture<#return_ty, #throw_ty>>,
                waker: ::std::option::Option<::uniffi::RustFutureForeignWakerFunction>,
                waker_environment: *const ::uniffi::RustFutureForeignWakerEnvironment,
                polled_result: &mut <#return_ty as ::uniffi::FfiReturn>::FfiType,
                call_status: &mut ::uniffi::RustCallStatus,
            ) -> bool {
                ::uniffi::ffi::uniffi_rustfuture_poll(future, waker, waker_environment, polled_result, call_status)
            }
        });

        // Monomorphised drop function.
        extra_functions.push(quote! {
            #[doc(hidden)]
            #[no_mangle]
            pub extern "C" fn #ffi_drop_ident(
                future: ::std::option::Option<::std::boxed::Box<::uniffi::RustFuture<#return_ty, #throw_ty>>>,
                call_status: &mut ::uniffi::RustCallStatus,
            ) {
                ::uniffi::ffi::uniffi_rustfuture_drop(future, call_status)
            }
        });
    }

    let argument_error = match &arguments.async_runtime {
        Some(async_runtime) if !is_async => Some(
            syn::Error::new(
                async_runtime.span(),
                "this attribute is only allowed on async functions",
            )
            .into_compile_error(),
        ),
        _ => None,
    };

    quote! {
        #[doc(hidden)]
        #[no_mangle]
        pub extern "C" fn #ffi_ident(
            #(#params,)*
            call_status: &mut ::uniffi::RustCallStatus,
        ) -> #return_expr {
            ::uniffi::deps::log::debug!(#name);
            #body_expr
        }

        #( #extra_functions )*

        #argument_error
    }
}
