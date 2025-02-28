/// `fixture`'s related data and parsing
use syn::{
    parse::{Parse, ParseStream},
    parse_quote,
    visit_mut::VisitMut,
    Expr, FnArg, Ident, ItemFn, Token,
};

use super::{
    extract_argument_attrs, extract_default_return_type, extract_defaults, extract_fixtures,
    extract_partials_return_type, parse_vector_trailing_till_double_comma, Attributes,
    ExtendWithFunctionAttrs, Fixture, Positional,
};
use crate::parse::Attribute;
use crate::{error::ErrorsVec, refident::RefIdent, utils::attr_is};
use proc_macro2::TokenStream;
use quote::{format_ident, ToTokens};

#[derive(PartialEq, Debug, Default)]
pub(crate) struct FixtureInfo {
    pub(crate) data: FixtureData,
    pub(crate) attributes: FixtureModifiers,
}

impl Parse for FixtureModifiers {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(input.parse::<Attributes>()?.into())
    }
}

impl Parse for FixtureInfo {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(if input.is_empty() {
            Default::default()
        } else {
            Self {
                data: input.parse()?,
                attributes: input
                    .parse::<Token![::]>()
                    .or_else(|_| Ok(Default::default()))
                    .and_then(|_| input.parse())?,
            }
        })
    }
}

impl ExtendWithFunctionAttrs for FixtureInfo {
    fn extend_with_function_attrs(
        &mut self,
        item_fn: &mut ItemFn,
    ) -> std::result::Result<(), ErrorsVec> {
        let composed_tuple!(
            fixtures,
            defaults,
            default_return_type,
            partials_return_type
        ) = merge_errors!(
            extract_fixtures(item_fn),
            extract_defaults(item_fn),
            extract_default_return_type(item_fn),
            extract_partials_return_type(item_fn)
        )?;
        self.data.items.extend(
            fixtures
                .into_iter()
                .map(|f| f.into())
                .chain(defaults.into_iter().map(|d| d.into())),
        );
        if let Some(return_type) = default_return_type {
            self.attributes.set_default_return_type(return_type);
        }
        for (id, return_type) in partials_return_type {
            self.attributes.set_partial_return_type(id, return_type);
        }
        Ok(())
    }
}

/// Simple struct used to visit function attributes and extract Fixtures and
/// eventualy parsing errors
#[derive(Default)]
pub(crate) struct FixturesFunctionExtractor(pub(crate) Vec<Fixture>, pub(crate) Vec<syn::Error>);

impl VisitMut for FixturesFunctionExtractor {
    fn visit_fn_arg_mut(&mut self, node: &mut FnArg) {
        for r in extract_argument_attrs(
            node,
            |a| attr_is(a, "with"),
            |a, name| {
                a.parse_args::<Positional>()
                    .map(|p| Fixture::new(name.clone(), p))
            },
        ) {
            match r {
                Ok(fixture) => self.0.push(fixture),
                Err(err) => self.1.push(err),
            }
        }
    }
}

/// Simple struct used to visit function attributes and extract fixture default values info and
/// eventualy parsing errors
#[derive(Default)]
pub(crate) struct DefaultsFunctionExtractor(
    pub(crate) Vec<ArgumentValue>,
    pub(crate) Vec<syn::Error>,
);

impl VisitMut for DefaultsFunctionExtractor {
    fn visit_fn_arg_mut(&mut self, node: &mut FnArg) {
        for r in extract_argument_attrs(
            node,
            |a| attr_is(a, "default"),
            |a, name| {
                a.parse_args::<Expr>()
                    .map(|e| ArgumentValue::new(name.clone(), e))
            },
        ) {
            match r {
                Ok(value) => self.0.push(value),
                Err(err) => self.1.push(err),
            }
        }
    }
}

#[derive(PartialEq, Debug, Default)]
pub(crate) struct FixtureData {
    pub items: Vec<FixtureItem>,
}

impl FixtureData {
    pub(crate) fn fixtures(&self) -> impl Iterator<Item = &Fixture> {
        self.items.iter().filter_map(|f| match f {
            FixtureItem::Fixture(ref fixture) => Some(fixture),
            _ => None,
        })
    }

    pub(crate) fn values(&self) -> impl Iterator<Item = &ArgumentValue> {
        self.items.iter().filter_map(|f| match f {
            FixtureItem::ArgumentValue(ref value) => Some(value),
            _ => None,
        })
    }
}

impl Parse for FixtureData {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![::]) {
            Ok(Default::default())
        } else {
            Ok(Self {
                items: parse_vector_trailing_till_double_comma::<_, Token![,]>(input)?,
            })
        }
    }
}

#[derive(PartialEq, Debug)]
pub(crate) struct ArgumentValue {
    pub name: Ident,
    pub expr: Expr,
}

impl ArgumentValue {
    pub(crate) fn new(name: Ident, expr: Expr) -> Self {
        Self { name, expr }
    }
}

#[derive(PartialEq, Debug)]
pub(crate) enum FixtureItem {
    Fixture(Fixture),
    ArgumentValue(ArgumentValue),
}

impl From<Fixture> for FixtureItem {
    fn from(f: Fixture) -> Self {
        FixtureItem::Fixture(f)
    }
}

impl Parse for FixtureItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek2(Token![=]) {
            input.parse::<ArgumentValue>().map(|v| v.into())
        } else {
            input.parse::<Fixture>().map(|v| v.into())
        }
    }
}

impl RefIdent for FixtureItem {
    fn ident(&self) -> &Ident {
        match self {
            FixtureItem::Fixture(Fixture { ref name, .. }) => name,
            FixtureItem::ArgumentValue(ArgumentValue { ref name, .. }) => name,
        }
    }
}

impl ToTokens for FixtureItem {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.ident().to_tokens(tokens)
    }
}

impl From<ArgumentValue> for FixtureItem {
    fn from(av: ArgumentValue) -> Self {
        FixtureItem::ArgumentValue(av)
    }
}

impl Parse for ArgumentValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _eq: Token![=] = input.parse()?;
        let expr = input.parse()?;
        Ok(ArgumentValue::new(name, expr))
    }
}

wrap_attributes!(FixtureModifiers);

impl FixtureModifiers {
    pub(crate) const DEFAULT_RET_ATTR: &'static str = "default";
    pub(crate) const PARTIAL_RET_ATTR: &'static str = "partial_";

    pub(crate) fn extract_default_type(&self) -> Option<syn::ReturnType> {
        self.extract_type(Self::DEFAULT_RET_ATTR)
    }

    pub(crate) fn extract_partial_type(&self, pos: usize) -> Option<syn::ReturnType> {
        self.extract_type(&format!("{}{}", Self::PARTIAL_RET_ATTR, pos))
    }

    pub(crate) fn set_default_return_type(&mut self, return_type: syn::Type) {
        self.inner.attributes.push(Attribute::Type(
            format_ident!("{}", Self::DEFAULT_RET_ATTR),
            return_type,
        ))
    }

    pub(crate) fn set_partial_return_type(&mut self, id: usize, return_type: syn::Type) {
        self.inner.attributes.push(Attribute::Type(
            format_ident!("{}{}", Self::PARTIAL_RET_ATTR, id),
            return_type,
        ))
    }

    fn extract_type(&self, attr_name: &str) -> Option<syn::ReturnType> {
        self.iter()
            .filter_map(|m| match m {
                Attribute::Type(name, t) if name == attr_name => Some(parse_quote! { -> #t}),
                _ => None,
            })
            .next()
    }
}

#[cfg(test)]
mod should {
    use super::*;
    use crate::test::{assert_eq, *};

    mod parse {
        use super::{assert_eq, *};
        use mytest::rstest;

        fn parse_fixture<S: AsRef<str>>(fixture_data: S) -> FixtureInfo {
            parse_meta(fixture_data)
        }

        #[test]
        fn happy_path() {
            let data = parse_fixture(
                r#"my_fixture(42, "other"), other(vec![42]), value=42, other_value=vec![1.0]
                    :: trace :: no_trace(some)"#,
            );

            let expected = FixtureInfo {
                data: vec![
                    fixture("my_fixture", vec!["42", r#""other""#]).into(),
                    fixture("other", vec!["vec![42]"]).into(),
                    arg_value("value", "42").into(),
                    arg_value("other_value", "vec![1.0]").into(),
                ]
                .into(),
                attributes: Attributes {
                    attributes: vec![
                        Attribute::attr("trace"),
                        Attribute::tagged("no_trace", vec!["some"]),
                    ],
                }
                .into(),
            };

            assert_eq!(expected, data);
        }

        #[test]
        fn some_literals() {
            let args_expressions = literal_expressions_str();
            let fixture = parse_fixture(&format!("my_fixture({})", args_expressions.join(", ")));
            let args = fixture.data.fixtures().next().unwrap().positional.clone();

            assert_eq!(to_args!(args_expressions), args.0);
        }

        #[test]
        fn empty_fixtures() {
            let data = parse_fixture(r#"::trace::no_trace(some)"#);

            let expected = FixtureInfo {
                attributes: Attributes {
                    attributes: vec![
                        Attribute::attr("trace"),
                        Attribute::tagged("no_trace", vec!["some"]),
                    ],
                }
                .into(),
                ..Default::default()
            };

            assert_eq!(expected, data);
        }

        #[test]
        fn empty_attributes() {
            let data = parse_fixture(r#"my_fixture(42, "other")"#);

            let expected = FixtureInfo {
                data: vec![fixture("my_fixture", vec!["42", r#""other""#]).into()].into(),
                ..Default::default()
            };

            assert_eq!(expected, data);
        }

        #[rstest]
        #[case("first(42),", 1)]
        #[case("first(42), second=42,", 2)]
        #[case(r#"fixture(42, "other"), :: trace"#, 1)]
        #[case(r#"second=42, fixture(42, "other"), :: trace"#, 2)]
        fn should_accept_trailing_comma(#[case] input: &str, #[case] expected: usize) {
            let info: FixtureInfo = input.ast();

            assert_eq!(
                expected,
                info.data.fixtures().count() + info.data.values().count()
            );
        }
    }
}

#[cfg(test)]
mod extend {
    use super::*;
    use crate::test::{assert_eq, *};
    use syn::ItemFn;

    mod should {
        use super::{assert_eq, *};

        #[test]
        fn use_with_attributes() {
            let to_parse = r#"
                fn my_fix(#[with(2)] f1: &str, #[with(vec![1,2], "s")] f2: u32) {}
            "#;

            let mut item_fn: ItemFn = to_parse.ast();
            let mut info = FixtureInfo::default();

            info.extend_with_function_attrs(&mut item_fn).unwrap();

            let expected = FixtureInfo {
                data: vec![
                    fixture("f1", vec!["2"]).into(),
                    fixture("f2", vec!["vec![1,2]", r#""s""#]).into(),
                ]
                .into(),
                ..Default::default()
            };

            assert!(!format!("{:?}", item_fn).contains("with"));
            assert_eq!(expected, info);
        }

        #[test]
        fn use_default_values_attributes() {
            let to_parse = r#"
                fn my_fix(#[default(2)] f1: &str, #[default((vec![1,2], "s"))] f2: (Vec<u32>, &str)) {}
            "#;

            let mut item_fn: ItemFn = to_parse.ast();
            let mut info = FixtureInfo::default();

            info.extend_with_function_attrs(&mut item_fn).unwrap();

            let expected = FixtureInfo {
                data: vec![
                    arg_value("f1", "2").into(),
                    arg_value("f2", r#"(vec![1,2], "s")"#).into(),
                ]
                .into(),
                ..Default::default()
            };

            assert!(!format!("{:?}", item_fn).contains("default"));
            assert_eq!(expected, info);
        }

        #[test]
        fn find_default_return_type() {
            let mut item_fn: ItemFn = r#"
                #[simple]
                #[first(comp)]
                #[second::default]
                #[default(impl Iterator<Item=(u32, i32)>)]
                #[last::more]
                fn my_fix<I, J>(f1: I, f2: J) -> impl Iterator<Item=(I, J)> {}
            "#
            .ast();

            let mut info = FixtureInfo::default();

            info.extend_with_function_attrs(&mut item_fn).unwrap();

            assert_eq!(
                info.attributes.extract_default_type(),
                Some(parse_quote! { -> impl Iterator<Item=(u32, i32)> })
            );
            assert_eq!(
                attrs("#[simple]#[first(comp)]#[second::default]#[last::more]"),
                item_fn.attrs
            );
        }

        #[test]
        fn find_partials_return_type() {
            let mut item_fn: ItemFn = r#"
                #[simple]
                #[first(comp)]
                #[second::default]
                #[partial_1(impl Iterator<Item=(u32, J, K)>)]
                #[partial_2(impl Iterator<Item=(u32, i32, K)>)]
                #[last::more]
                fn my_fix<I, J, K>(f1: I, f2: J, f3: K) -> impl Iterator<Item=(I, J, K)> {}
            "#
            .ast();

            let mut info = FixtureInfo::default();

            info.extend_with_function_attrs(&mut item_fn).unwrap();

            assert_eq!(
                info.attributes.extract_partial_type(1),
                Some(parse_quote! { -> impl Iterator<Item=(u32, J, K)> })
            );
            assert_eq!(
                info.attributes.extract_partial_type(2),
                Some(parse_quote! { -> impl Iterator<Item=(u32, i32, K)> })
            );
            assert_eq!(
                attrs("#[simple]#[first(comp)]#[second::default]#[last::more]"),
                item_fn.attrs
            );
        }

        mod raise_error {
            use super::{assert_eq, *};
            use rstest_test::assert_in;

            #[test]
            fn for_invalid_expressions() {
                let mut item_fn: ItemFn = r#"
                fn my_fix(#[with(valid)] f1: &str, #[with(with(,.,))] f2: u32, #[with(with(use))] f3: u32) {}
                "#
                .ast();

                let errors = FixtureInfo::default()
                    .extend_with_function_attrs(&mut item_fn)
                    .unwrap_err();

                assert_eq!(2, errors.len());
            }

            #[test]
            fn for_invalid_default_type() {
                let mut item_fn: ItemFn = r#"
                    #[default(no<valid::>type)]
                    fn my_fix<I>() -> I {}
                "#
                .ast();

                let errors = FixtureInfo::default()
                    .extend_with_function_attrs(&mut item_fn)
                    .unwrap_err();

                assert_eq!(1, errors.len());
            }

            #[test]
            fn if_default_is_defined_more_than_once() {
                let mut item_fn: ItemFn = r#"
                    #[default(u32)]
                    #[default(u32)]
                    fn my_fix<I>() -> I {}
                    "#
                .ast();

                let mut info = FixtureInfo::default();

                let error = info.extend_with_function_attrs(&mut item_fn).unwrap_err();

                assert_in!(
                    format!("{:?}", error).to_lowercase(),
                    "cannot use default more than once"
                );
            }

            #[test]
            fn for_invalid_partial_type() {
                let mut item_fn: ItemFn = r#"
                    #[partial_1(no<valid::>type)]
                    fn my_fix<I>(x: I, y: u32) -> I {}
                "#
                .ast();

                let errors = FixtureInfo::default()
                    .extend_with_function_attrs(&mut item_fn)
                    .unwrap_err();

                assert_eq!(1, errors.len());
            }

            #[test]
            fn if_partial_is_not_correct() {
                let mut item_fn: ItemFn = r#"
                    #[partial_not_a_number(u32)]
                    fn my_fix<I, J>(f1: I, f2: &str) -> I {}
                    "#
                .ast();

                let mut info = FixtureInfo::default();

                let error = info.extend_with_function_attrs(&mut item_fn).unwrap_err();

                assert_in!(
                    format!("{:?}", error).to_lowercase(),
                    "invalid partial syntax"
                );
            }
        }
    }
}
