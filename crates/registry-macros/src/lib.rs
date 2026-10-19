//! # tt-registry-macros — the `#[command]` attribute
//!
//! Declares a command next to its handler (plan §3.2, 11 §2). Applied to a handler function
//! `fn name(cx: &mut Cx, params: Params) -> Result<Output, CommandError>`, it keeps the function and adds a
//! `static NAME: tt_registry::Command` (the function name in upper case) holding the declaration — id, kind, label,
//! scopes, undo label, confirmation conditions, enablement rule, project requirement, examples, the parameter and
//! result schemas derived from `Params` and `Output`, and the implementation status — and a type-erased handler that
//! decodes the parameters, downcasts the context to `Cx`, runs the function and encodes the result.
//!
//! ```text
//! #[command(
//!     id = "edit.undo", kind = action, label = "Undo", scopes = [edit],
//!     enablement = rules::CAN_UNDO, needs_project,
//!     example(title = "Undo the last step", params = r#"{"steps": 1}"#),
//! )]
//! fn edit_undo(cx: &mut Cx, p: UndoParams) -> Result<UndoResult, CommandError> { … }
//! ```
//!
//! Keys: `id`, `kind` (`action` `toggle` `query` `job` `ui` `event`), `label` (required); `scopes = [...]`
//! (`read` `edit` `ui` `input` `files_read` `files_write` `prefs` `library_global` `network` `print` `app`); `undo`
//! (undo label of an undoable command); `confirm = ["condition", …]`; `enablement = PATH` (a `const`
//! `tt_registry::Enablement`; default: always enabled); `needs_project`; `example(title, params)` (repeatable);
//! `not_implemented = "M1 C1.3"` (declared now, implemented by the named checkpoint: the handler validates and decodes
//! the parameters, then fails with `command.notImplemented`); `agent_only = "justification"` (11 §14.1); `gui = "…"`
//! (the GUI locations that invoke the command besides the menu manifest, for the parity test).

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, ExprArray, ExprLit, FnArg, ItemFn, Lit, Meta, Pat, PathArguments, ReturnType, Token, Type};

/// Declares a command; see the crate documentation.
#[proc_macro_attribute]
pub fn command(attr: TokenStream, item: TokenStream) -> TokenStream {
    match expand(attr.into(), item.into()) {
        Ok(t) => t.into(),
        Err(e) => e.to_compile_error().into(),
    }
}

#[derive(Default)]
struct Args {
    id: Option<String>,
    kind: Option<syn::Ident>,
    label: Option<String>,
    scopes: Vec<syn::Ident>,
    undo: Option<String>,
    confirm: Vec<String>,
    enablement: Option<syn::Path>,
    needs_project: bool,
    examples: Vec<(String, String)>,
    not_implemented: Option<String>,
    agent_only: Option<String>,
    gui: Option<String>,
}

fn lit_str(e: &Expr) -> syn::Result<String> {
    match e {
        Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) => Ok(s.value()),
        other => Err(syn::Error::new_spanned(other, "expected a string literal")),
    }
}

fn ident(e: &Expr) -> syn::Result<syn::Ident> {
    match e {
        Expr::Path(p) => {
            p.path.get_ident().cloned().ok_or_else(|| syn::Error::new_spanned(p, "expected an identifier"))
        }
        other => Err(syn::Error::new_spanned(other, "expected an identifier")),
    }
}

fn parse_args(attr: TokenStream2) -> syn::Result<Args> {
    let metas = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr)?;
    let mut a = Args::default();
    for m in metas {
        match &m {
            Meta::Path(p) if p.is_ident("needs_project") => a.needs_project = true,
            Meta::NameValue(nv) => {
                let key = nv.path.get_ident().map(ToString::to_string).unwrap_or_default();
                match key.as_str() {
                    "id" => a.id = Some(lit_str(&nv.value)?),
                    "kind" => a.kind = Some(ident(&nv.value)?),
                    "label" => a.label = Some(lit_str(&nv.value)?),
                    "undo" => a.undo = Some(lit_str(&nv.value)?),
                    "not_implemented" => a.not_implemented = Some(lit_str(&nv.value)?),
                    "agent_only" => a.agent_only = Some(lit_str(&nv.value)?),
                    "gui" => a.gui = Some(lit_str(&nv.value)?),
                    "enablement" => match &nv.value {
                        Expr::Path(p) => a.enablement = Some(p.path.clone()),
                        other => return Err(syn::Error::new_spanned(other, "expected a path to an Enablement")),
                    },
                    "scopes" | "confirm" => {
                        let Expr::Array(ExprArray { elems, .. }) = &nv.value else {
                            return Err(syn::Error::new_spanned(&nv.value, "expected [..]"));
                        };
                        for el in elems {
                            if key == "scopes" {
                                a.scopes.push(ident(el)?);
                            } else {
                                a.confirm.push(lit_str(el)?);
                            }
                        }
                    }
                    _ => return Err(syn::Error::new_spanned(&nv.path, format!("unknown key `{key}`"))),
                }
            }
            Meta::List(l) if l.path.is_ident("example") => {
                let inner = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(l.tokens.clone())?;
                let (mut title, mut params) = (None, None);
                for im in inner {
                    let Meta::NameValue(nv) = im else {
                        return Err(syn::Error::new_spanned(im, "expected `title = \"…\"` or `params = \"…\"`"));
                    };
                    if nv.path.is_ident("title") {
                        title = Some(lit_str(&nv.value)?);
                    } else if nv.path.is_ident("params") {
                        params = Some(lit_str(&nv.value)?);
                    } else {
                        return Err(syn::Error::new_spanned(&nv.path, "unknown example key"));
                    }
                }
                let title = title.ok_or_else(|| syn::Error::new_spanned(l, "example needs `title`"))?;
                let params = params.unwrap_or_else(|| "{}".to_owned());
                a.examples.push((title, params));
            }
            other => return Err(syn::Error::new_spanned(other, "unexpected attribute argument")),
        }
    }
    Ok(a)
}

/// `&mut T` → `T`.
fn context_type(arg: &FnArg) -> syn::Result<Type> {
    let FnArg::Typed(pt) = arg else { return Err(syn::Error::new_spanned(arg, "expected `cx: &mut Context`")) };
    match &*pt.ty {
        Type::Reference(r) if r.mutability.is_some() => Ok((*r.elem).clone()),
        other => Err(syn::Error::new_spanned(other, "the first argument must be `&mut Context`")),
    }
}

fn params_type(arg: &FnArg) -> syn::Result<Type> {
    let FnArg::Typed(pt) = arg else { return Err(syn::Error::new_spanned(arg, "expected `params: Params`")) };
    if !matches!(&*pt.pat, Pat::Ident(_) | Pat::Wild(_)) {
        return Err(syn::Error::new_spanned(&pt.pat, "expected a parameter name"));
    }
    Ok((*pt.ty).clone())
}

/// `Result<T, E>` → `T`.
fn output_type(ret: &ReturnType) -> syn::Result<Type> {
    let ReturnType::Type(_, ty) = ret else {
        return Err(syn::Error::new_spanned(ret, "the handler must return Result<Output, CommandError>"));
    };
    if let Type::Path(p) = &**ty
        && let Some(seg) = p.path.segments.last()
        && seg.ident == "Result"
        && let PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(syn::GenericArgument::Type(t)) = args.args.first()
    {
        return Ok(t.clone());
    }
    Err(syn::Error::new_spanned(ty, "the handler must return Result<Output, CommandError>"))
}

fn expand(attr: TokenStream2, item: TokenStream2) -> syn::Result<TokenStream2> {
    let a = parse_args(attr)?;
    let f: ItemFn = syn::parse2(item)?;
    let span = Span::call_site();
    let id = a.id.ok_or_else(|| syn::Error::new(span, "`id` is required"))?;
    let label = a.label.ok_or_else(|| syn::Error::new(span, "`label` is required"))?;
    let kind = a.kind.ok_or_else(|| syn::Error::new(span, "`kind` is required"))?;
    let kind_variant = match kind.to_string().as_str() {
        "action" => quote!(Action),
        "toggle" => quote!(Toggle),
        "query" => quote!(Query),
        "job" => quote!(Job),
        "ui" => quote!(Ui),
        "event" => quote!(Event),
        _ => return Err(syn::Error::new_spanned(kind, "unknown kind")),
    };
    let mut scopes = Vec::new();
    for s in &a.scopes {
        scopes.push(match s.to_string().as_str() {
            "read" => quote!(Read),
            "edit" => quote!(Edit),
            "ui" => quote!(Ui),
            "input" => quote!(Input),
            "files_read" => quote!(FilesRead),
            "files_write" => quote!(FilesWrite),
            "prefs" => quote!(Prefs),
            "library_global" => quote!(LibraryGlobal),
            "network" => quote!(Network),
            "print" => quote!(Print),
            "app" => quote!(App),
            _ => return Err(syn::Error::new_spanned(s, "unknown scope")),
        });
    }
    if a.examples.is_empty() {
        return Err(syn::Error::new(span, "at least one `example(...)` is required (11 §14.2)"));
    }
    let inputs: Vec<&FnArg> = f.sig.inputs.iter().collect();
    let [cx_arg, p_arg] = inputs.as_slice() else {
        return Err(syn::Error::new_spanned(&f.sig, "a handler takes (cx: &mut Context, params: Params)"));
    };
    let cx_ty = context_type(cx_arg)?;
    let p_ty = params_type(p_arg)?;
    let out_ty = output_type(&f.sig.output)?;
    let fn_name = &f.sig.ident;
    let static_name = format_ident!("{}", fn_name.to_string().to_uppercase());
    let vis = &f.vis;
    let undo = match &a.undo {
        Some(u) => quote!(::core::option::Option::Some(#u)),
        None => quote!(::core::option::Option::None),
    };
    let agent_only = match &a.agent_only {
        Some(u) => quote!(::core::option::Option::Some(#u)),
        None => quote!(::core::option::Option::None),
    };
    let gui = match &a.gui {
        Some(u) => quote!(::core::option::Option::Some(#u)),
        None => quote!(::core::option::Option::None),
    };
    let confirm = &a.confirm;
    let enablement = match &a.enablement {
        Some(p) => quote!(#p),
        None => quote!(::tt_registry::Enablement { rule: "always enabled", check: ::core::option::Option::None }),
    };
    let needs_project = a.needs_project;
    let example_titles = a.examples.iter().map(|e| &e.0);
    let example_params = a.examples.iter().map(|e| &e.1);
    let (status, body) = match &a.not_implemented {
        Some(owner) => (
            quote!(::tt_registry::Status::NotImplemented { owner: #owner }),
            quote! {
                let _ = ::tt_registry::context::<#cx_ty>(cx)?;
                let _params: #p_ty = ::tt_registry::decode_params(v)?;
                ::core::result::Result::Err(::tt_registry::CommandError::not_implemented(#id, #owner))
            },
        ),
        None => (
            quote!(::tt_registry::Status::Implemented),
            quote! {
                let cx = ::tt_registry::context::<#cx_ty>(cx)?;
                let params: #p_ty = ::tt_registry::decode_params(v)?;
                let out: #out_ty = #fn_name(cx, params)?;
                ::tt_registry::encode_result(&out)
            },
        ),
    };
    let allow_dead = if a.not_implemented.is_some() { quote!(#[allow(dead_code)]) } else { quote!() };
    let doc = format!("Declaration of `{id}` ({label}).");
    Ok(quote! {
        #allow_dead
        #f

        #[doc = #doc]
        #vis static #static_name: ::tt_registry::Command = ::tt_registry::Command {
            decl: ::tt_registry::Decl {
                id: #id,
                kind: ::tt_registry::Kind::#kind_variant,
                label: #label,
                scopes: &[#(::tt_registry::Scope::#scopes),*],
                undo: #undo,
                confirm: &[#(#confirm),*],
                enablement: #enablement,
                needs_project: #needs_project,
                examples: &[#(::tt_registry::Example { title: #example_titles, params: #example_params }),*],
                params_schema: ::tt_registry::schema_of::<#p_ty>,
                result_schema: ::tt_registry::schema_of::<#out_ty>,
                status: #status,
                agent_only: #agent_only,
                gui: #gui,
            },
            handler: {
                fn __handler(
                    cx: &mut dyn ::core::any::Any,
                    v: ::serde_json::Value,
                ) -> ::core::result::Result<::serde_json::Value, ::tt_registry::CommandError> {
                    #body
                }
                __handler
            },
        };
    })
}
