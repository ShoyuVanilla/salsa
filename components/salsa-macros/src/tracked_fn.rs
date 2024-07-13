use proc_macro2::{Span, TokenStream};
use quote::ToTokens;
use syn::{spanned::Spanned, ItemFn};

use crate::{db_lifetime, hygiene::Hygiene, options::Options};

// Source:
//
// #[salsa::db]
// pub struct Database {
//    storage: salsa::Storage<Self>,
// }

pub(crate) fn tracked_fn(args: proc_macro::TokenStream, item: ItemFn) -> syn::Result<TokenStream> {
    let hygiene = Hygiene::from2(&item);
    let args: FnArgs = syn::parse(args)?;
    let db_macro = Macro { hygiene, args };
    db_macro.try_fn(item)
}

type FnArgs = Options<TrackedFn>;

struct TrackedFn;

impl crate::options::AllowedOptions for TrackedFn {
    const RETURN_REF: bool = true;

    const SPECIFY: bool = true;

    const NO_EQ: bool = true;

    const SINGLETON: bool = false;

    const JAR: bool = false;

    const DATA: bool = false;

    const DB: bool = false;

    const RECOVERY_FN: bool = true;

    const LRU: bool = true;

    const CONSTRUCTOR_NAME: bool = false;
}

struct Macro {
    hygiene: Hygiene,
    args: FnArgs,
}

struct ValidFn<'item> {
    db_ident: &'item syn::Ident,
    db_path: &'item syn::Path,
}

#[allow(non_snake_case)]
impl Macro {
    fn try_fn(&self, item: syn::ItemFn) -> syn::Result<TokenStream> {
        let ValidFn { db_ident, db_path } = self.validity_check(&item)?;

        let fn_name = &item.sig.ident;
        let vis = &item.vis;
        let db_lt = db_lifetime::db_lifetime(&item.sig.generics);
        let input_ids = self.input_ids(&item);
        let input_tys = self.input_tys(&item)?;
        let output_ty = self.output_ty(&item)?;
        let (cycle_recovery_fn, cycle_recovery_strategy) = self.cycle_recovery();

        let mut inner_fn = item.clone();
        inner_fn.vis = syn::Visibility::Inherited;
        inner_fn.sig.ident = self.hygiene.ident("inner");

        let zalsa = self.hygiene.ident("zalsa");
        let Configuration = self.hygiene.ident("Configuration");
        let InternedData = self.hygiene.ident("InternedData");
        let FN_CACHE = self.hygiene.ident("FN_CACHE");
        let INTERN_CACHE = self.hygiene.ident("INTERN_CACHE");
        let inner = &inner_fn.sig.ident;

        match function_type(&item) {
            FunctionType::RequiresInterning => Ok(crate::debug::dump_tokens(
                fn_name,
                quote![salsa::plumbing::setup_interned_fn! {
                    vis: #vis,
                    fn_name: #fn_name,
                    db_lt: #db_lt,
                    Db: #db_path,
                    db: #db_ident,
                    input_ids: [#(#input_ids),*],
                    input_tys: [#(#input_tys),*],
                    output_ty: #output_ty,
                    inner_fn: #inner_fn,
                    cycle_recovery_fn: #cycle_recovery_fn,
                    cycle_recovery_strategy: #cycle_recovery_strategy,
                    unused_names: [
                        #zalsa,
                        #Configuration,
                        #InternedData,
                        #FN_CACHE,
                        #INTERN_CACHE,
                        #inner,
                    ]
                }],
            )),
            FunctionType::Constant => todo!(),
            FunctionType::SalsaStruct => todo!(),
        }
    }

    fn validity_check<'item>(&self, item: &'item syn::ItemFn) -> syn::Result<ValidFn<'item>> {
        db_lifetime::require_optional_db_lifetime(&item.sig.generics)?;

        if item.sig.inputs.is_empty() {
            return Err(syn::Error::new_spanned(
                &item.sig.ident,
                "tracked functions must have at least a database argument",
            ));
        }

        let (db_ident, db_path) = self.check_db_argument(&item.sig.inputs[0])?;

        Ok(ValidFn { db_ident, db_path })
    }

    fn check_db_argument<'arg>(
        &self,
        fn_arg: &'arg syn::FnArg,
    ) -> syn::Result<(&'arg syn::Ident, &'arg syn::Path)> {
        match fn_arg {
            syn::FnArg::Receiver(_) => {
                // If we see `&self` where a database was expected, that indicates
                // that `#[tracked]` was applied to a method.
                return Err(syn::Error::new_spanned(
                    fn_arg,
                    "#[salsa::tracked] must also be applied to the impl block for tracked methods",
                ));
            }
            syn::FnArg::Typed(typed) => {
                let syn::Pat::Ident(db_pat_ident) = &*typed.pat else {
                    return Err(syn::Error::new_spanned(
                        &typed.pat,
                        "database parameter must have a simple name",
                    ));
                };

                let syn::PatIdent {
                    attrs,
                    by_ref,
                    mutability,
                    ident: db_ident,
                    subpat,
                } = db_pat_ident;

                if !attrs.is_empty() {
                    return Err(syn::Error::new_spanned(
                        db_pat_ident,
                        "database parameter cannot have attributes",
                    ));
                }

                if by_ref.is_some() {
                    return Err(syn::Error::new_spanned(
                        by_ref,
                        "database parameter cannot be borrowed",
                    ));
                }

                if mutability.is_some() {
                    return Err(syn::Error::new_spanned(
                        mutability,
                        "database parameter cannot be mutable",
                    ));
                }

                if let Some((at, _)) = subpat {
                    return Err(syn::Error::new_spanned(
                        at,
                        "database parameter cannot have a subpattern",
                    ));
                }

                let extract_db_path = || -> Result<&'arg syn::Path, Span> {
                    let syn::Type::Reference(ref_type) = &*typed.ty else {
                        return Err(typed.ty.span());
                    };

                    if let Some(m) = &ref_type.mutability {
                        return Err(m.span());
                    }

                    let syn::Type::TraitObject(d) = &*ref_type.elem else {
                        return Err(ref_type.span());
                    };

                    if d.bounds.len() != 1 {
                        return Err(d.span());
                    }

                    let syn::TypeParamBound::Trait(syn::TraitBound {
                        paren_token,
                        modifier,
                        lifetimes,
                        path,
                    }) = &d.bounds[0]
                    else {
                        return Err(d.span());
                    };

                    if let Some(p) = paren_token {
                        return Err(p.span.open());
                    }

                    let syn::TraitBoundModifier::None = modifier else {
                        return Err(d.span());
                    };

                    if let Some(lt) = lifetimes {
                        return Err(lt.span());
                    }

                    Ok(path)
                };

                let db_path = extract_db_path().map_err(|span| {
                    syn::Error::new(
                        span,
                        "must have type `&dyn Db`, where `Db` is some Salsa Database trait",
                    )
                })?;

                Ok((db_ident, db_path))
            }
        }
    }

    /// Returns a vector of ids representing the function arguments.
    /// Prefers to reuse the names given by the user, if possible.
    fn input_ids(&self, item: &ItemFn) -> Vec<syn::Ident> {
        item.sig
            .inputs
            .iter()
            .skip(1)
            .zip(0..)
            .map(|(input, index)| {
                if let syn::FnArg::Typed(typed) = input {
                    if let syn::Pat::Ident(ident) = &*typed.pat {
                        return ident.ident.clone();
                    }
                }

                self.hygiene.ident(&format!("input{}", index))
            })
            .collect()
    }

    fn input_tys<'item>(&self, item: &'item ItemFn) -> syn::Result<Vec<&'item syn::Type>> {
        item.sig
            .inputs
            .iter()
            .skip(1)
            .map(|input| {
                if let syn::FnArg::Typed(typed) = input {
                    Ok(&*typed.ty)
                } else {
                    Err(syn::Error::new_spanned(input, "unexpected receiver"))
                }
            })
            .collect()
    }

    fn output_ty<'item>(&self, item: &'item ItemFn) -> syn::Result<syn::Type> {
        match &item.sig.output {
            syn::ReturnType::Default => Ok(parse_quote!("()")),
            syn::ReturnType::Type(_, ty) => Ok(syn::Type::clone(ty)),
        }
    }

    fn cycle_recovery(&self) -> (TokenStream, TokenStream) {
        if let Some(recovery_fn) = &self.args.recovery_fn {
            (recovery_fn.to_token_stream(), quote!(Fallback))
        } else {
            (
                quote!((salsa::plumbing::unexpected_cycle_recovery!)),
                quote!(Panic),
            )
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum FunctionType {
    Constant,
    SalsaStruct,
    RequiresInterning,
}

fn function_type(item_fn: &syn::ItemFn) -> FunctionType {
    match item_fn.sig.inputs.len() {
        0 => unreachable!(
            "functions have been checked to have at least a database argument by this point"
        ),
        1 => FunctionType::Constant,
        2 => FunctionType::SalsaStruct,
        _ => FunctionType::RequiresInterning,
    }
}