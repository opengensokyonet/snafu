use crate::{
    parse::{
        self,
        attr::{self, ErrorLocation},
        into_crate_root, into_transformation,
        tuple_struct_field_impl::parse_tuple_struct_field,
        AtMostOne, Attribute, CrateRoot, ProvideExpression, SourceFrom, SynErrors,
    },
    TupleStructInfo,
};

struct Attributes {
    display_bounds: Option<super::DisplayBounds>,
    error_bounds: Option<super::ErrorBounds>,
    error_compat_bounds: Option<super::ErrorCompatBounds>,
    crate_root: Option<CrateRoot>,
    provide_expressions: Vec<ProvideExpression>,
    source_from: Option<SourceFrom>,
}

impl Attributes {
    fn from_syn(attrs: &[syn::Attribute]) -> syn::Result<Self> {
        let location = ErrorLocation::OnTupleStruct;
        let mut errors = SynErrors::default();

        let mut display_bounds = AtMostOne::attribute(attr::DisplayBounds, location);
        let mut error_bounds = AtMostOne::attribute(attr::ErrorBounds, location);
        let mut error_compat_bounds = AtMostOne::attribute(attr::ErrorCompatBounds, location);
        let mut crate_roots = AtMostOne::attribute(attr::CrateRoot, location);
        let mut provide_expressions = Vec::new();
        let mut source_froms = AtMostOne::attribute(attr::SourceFrom, location);

        parse::syn_attrs(attrs, &mut errors, |errors, attr| {
            use Attribute::*;

            match attr {
                Backtrace(a) => errors.push_invalid_flag(a, location),
                ContextFlag(a) => errors.push_invalid_flag(a, location),
                ContextName(a) => errors.push_invalid(a, location),
                ContextSuffix(a) => errors.push_invalid(a, location),
                CrateRoot(a) => crate_roots.push(a),
                Display(a) => errors.push_invalid(a, location),
                DisplayBounds(a) => display_bounds.push(a),
                ErrorBounds(a) => error_bounds.push(a),
                ErrorCompatBounds(a) => error_compat_bounds.push(a),
                DocComment(_a) => { /* no-op */ }
                Implicit(a) => errors.push_invalid_flag(a, location),
                Module(a) => errors.push_invalid(a, location),
                ProvideFlag(a) => errors.push_invalid_flag(a, location),
                ProvideExpression(a) => provide_expressions.push(a),
                SourceFlag(a) => errors.push_invalid_flag(a, location),
                SourceFrom(a) => source_froms.push(a),
                Transparent(a) => errors.push_invalid_flag(a, location),
                Visibility(a) => errors.push_invalid(a, location),
                Whatever(a) => errors.push_invalid(a, location),
            }
        });

        let crate_root = crate_roots.finish_default(&mut errors);
        let display_bounds = display_bounds.finish_default(&mut errors);
        let error_bounds = error_bounds.finish_default(&mut errors);
        let error_compat_bounds = error_compat_bounds.finish_default(&mut errors);
        let source_from = source_froms.finish_default(&mut errors);

        errors.finish(Self {
            display_bounds,
            error_bounds,
            error_compat_bounds,
            crate_root,
            provide_expressions,
            source_from,
        })
    }
}

pub(crate) fn parse_tuple_struct(
    fields: &syn::FieldsUnnamed,
    name: &syn::Ident,
    generics: &syn::Generics,
    attrs: &[syn::Attribute],
    span: impl quote::ToTokens,
) -> syn::Result<TupleStructInfo> {
    let attrs = Attributes::from_syn(attrs);
    let field_ty = parse_tuple_struct_field(fields, span);

    let (attrs, field_ty) = join_syn_error!(attrs, field_ty)?;

    let Attributes {
        display_bounds,
        error_bounds,
        error_compat_bounds,
        crate_root,
        provide_expressions,
        source_from,
    } = attrs;

    let crate_root = into_crate_root(crate_root);
    let generics = generics.clone();
    let name = name.clone();
    let provides = provide_expressions
        .into_iter()
        .map(|p| p.into_provide())
        .collect();
    let transformation = into_transformation(source_from, field_ty, true);

    Ok(TupleStructInfo {
        display_bounds: display_bounds.map(|b| b.predicates.into_iter().collect()),
        error_bounds: error_bounds.map(|b| b.predicates.into_iter().collect()),
        error_compat_bounds: error_compat_bounds.map(|b| b.predicates.into_iter().collect()),
        crate_root,
        generics,
        name,
        provides,
        transformation,
    })
}
