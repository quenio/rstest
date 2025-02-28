use quote::{format_ident, ToTokens};
use syn::{parse_quote, visit_mut::VisitMut, FnArg, ItemFn, Lifetime};

use crate::{error::ErrorsVec, refident::MaybeIdent, utils::attr_is};

#[derive(Default)]
pub(crate) struct ReplaceFutureAttribute {
    lifetimes: Vec<Lifetime>,
    errors: Vec<syn::Error>,
}

impl ReplaceFutureAttribute {
    pub(crate) fn replace(item_fn: &mut ItemFn) -> Result<(), ErrorsVec> {
        let mut visitor = Self::default();
        visitor.visit_item_fn_mut(item_fn);
        if !visitor.lifetimes.is_empty() {
            let mut generics = visitor
                .lifetimes
                .into_iter()
                .map(|lt| {
                    syn::GenericParam::Lifetime(syn::LifetimeDef {
                        lifetime: lt,
                        attrs: Default::default(),
                        colon_token: None,
                        bounds: Default::default(),
                    })
                })
                .collect::<Vec<_>>();
            generics.extend(item_fn.sig.generics.params.iter().cloned());
            item_fn.sig.generics = parse_quote! {
                <#(#generics),*>
            };
        }
        if visitor.errors.is_empty() {
            Ok(())
        } else {
            Err(visitor.errors.into())
        }
    }
}

impl VisitMut for ReplaceFutureAttribute {
    fn visit_fn_arg_mut(&mut self, node: &mut FnArg) {
        let ident = node.maybe_ident().cloned().unwrap();
        match node {
            FnArg::Typed(t) => {
                let attrs = std::mem::take(&mut t.attrs);
                let (futures, attrs): (Vec<_>, Vec<_>) =
                    attrs.into_iter().partition(|a| attr_is(a, "future"));
                t.attrs = attrs;
                if futures.is_empty() {
                    return;
                } else if futures.len() > 1 {
                    self.errors.extend(futures.iter().skip(1).map(|attr| {
                        syn::Error::new_spanned(
                            attr.into_token_stream(),
                            format!("Cannot use #[future] more than once."),
                        )
                    }));
                    return;
                }
                let ty = &mut t.ty;
                use syn::Type::*;
                match ty.as_ref() {
                    Group(_) | ImplTrait(_) | Infer(_) | Macro(_) | Never(_) | Slice(_)
                    | TraitObject(_) | Verbatim(_) => {
                        self.errors.push(syn::Error::new_spanned(
                            ty.into_token_stream(),
                            format!("This type cannot used to generete impl Future."),
                        ));
                        return;
                    }
                    _ => {}
                };
                if let Reference(tr) = ty.as_mut() {
                    if tr.lifetime.is_none() {
                        let lifetime = syn::Lifetime {
                            apostrophe: ident.span(),
                            ident: format_ident!("_{}", ident),
                        };
                        self.lifetimes.push(lifetime.clone());
                        tr.lifetime = lifetime.into();
                    }
                }

                t.ty = parse_quote! {
                    impl std::future::Future<Output = #ty>
                }
            }
            FnArg::Receiver(_) => {}
        }
    }
}

#[cfg(test)]
mod should {
    use super::*;
    use crate::test::{assert_eq, *};
    use mytest::rstest;
    use rstest_test::assert_in;

    #[rstest]
    #[case("fn simple(a: u32) {}")]
    #[case("fn more(a: u32, b: &str) {}")]
    #[case("fn gen<S: AsRef<str>>(a: u32, b: S) {}")]
    #[case("fn attr(#[case] a: u32, #[values(1,2)] b: i32) {}")]
    fn not_change_anything_if_no_future_attribute_found(#[case] item_fn: &str) {
        let mut item_fn: ItemFn = item_fn.ast();
        let orig = item_fn.clone();

        ReplaceFutureAttribute::replace(&mut item_fn).unwrap();

        assert_eq!(orig, item_fn)
    }

    #[rstest]
    #[case::simple(
        "fn f(#[future] a: u32) {}",
        "fn f(a: impl std::future::Future<Output = u32>) {}"
    )]
    #[case::more_than_one(
        "fn f(#[future] a: u32, #[future] b: String, #[future] c: std::collection::HashMap<usize, String>) {}",
        r#"fn f(a: impl std::future::Future<Output = u32>, 
                b: impl std::future::Future<Output = String>, 
                c: impl std::future::Future<Output = std::collection::HashMap<usize, String>>) {}"#,
    )]
    #[case::just_one(
        "fn f(a: u32, #[future] b: String) {}",
        r#"fn f(a: u32, 
                b: impl std::future::Future<Output = String>) {}"#
    )]
    #[case::generics(
        "fn f<S: AsRef<str>>(#[future] a: S) {}",
        "fn f<S: AsRef<str>>(a: impl std::future::Future<Output = S>) {}"
    )]
    fn replace_basic_type(#[case] item_fn: &str, #[case] expected: &str) {
        let mut item_fn: ItemFn = item_fn.ast();
        let expected: ItemFn = expected.ast();

        ReplaceFutureAttribute::replace(&mut item_fn).unwrap();

        assert_eq!(expected, item_fn)
    }

    #[rstest]
    #[case::base(
        "fn f(#[future] ident_name: &u32) {}",
        "fn f<'_ident_name>(ident_name: impl std::future::Future<Output = &'_ident_name u32>) {}"
    )]
    #[case::lifetime_already_exists(
        "fn f<'b>(#[future] a: &'b u32) {}",
        "fn f<'b>(a: impl std::future::Future<Output = &'b u32>) {}"
    )]
    #[case::some_other_generics(
        "fn f<'b, IT: Iterator<Item=String + 'b>>(#[future] a: &u32, it: IT) {}",
        "fn f<'_a, 'b, IT: Iterator<Item=String + 'b>>(a: impl std::future::Future<Output = &'_a u32>, it: IT) {}"
    )]
    fn replace_reference_type(#[case] item_fn: &str, #[case] expected: &str) {
        let mut item_fn: ItemFn = item_fn.ast();
        let expected: ItemFn = expected.ast();

        ReplaceFutureAttribute::replace(&mut item_fn).unwrap();

        assert_eq!(expected, item_fn)
    }

    #[rstest]
    #[case::no_more_than_one("fn f(#[future] #[future] a: u32) {}", "more than once")]
    #[case::no_impl("fn f(#[future] a: impl AsRef<str>) {}", "generete impl Future")]
    #[case::no_slice("fn f(#[future] a: [i32]) {}", "generete impl Future")]
    fn raise_error(#[case] item_fn: &str, #[case] message: &str) {
        let mut item_fn: ItemFn = item_fn.ast();

        let err = ReplaceFutureAttribute::replace(&mut item_fn).unwrap_err();

        assert_in!(format!("{:?}", err), message);
    }
}
