//! Bounds belong to the generated capability, not to the error type or its constructors.
use crate::FieldContainer;
use proc_macro2::{TokenStream, TokenTree};
use quote::{quote, ToTokens};
use std::collections::{BTreeMap, BTreeSet};
use syn::{Expr, GenericArgument, Generics, Lit, PathArguments, Type, WherePredicate};

fn depends_on_generics(ty: &Type, generics: &Generics) -> bool {
    let names: BTreeSet<_> = generics
        .params
        .iter()
        .map(|p| match p {
            syn::GenericParam::Type(p) => p.ident.to_string(),
            syn::GenericParam::Lifetime(p) => p.lifetime.ident.to_string(),
            syn::GenericParam::Const(p) => p.ident.to_string(),
        })
        .collect();
    fn contains(tokens: TokenStream, names: &BTreeSet<String>) -> bool {
        tokens.into_iter().any(|t| match t {
            TokenTree::Ident(i) => names.contains(&i.to_string()),
            TokenTree::Group(g) => contains(g.stream(), names),
            _ => false,
        })
    }
    contains(ty.to_token_stream(), &names)
}

fn inner_type<'a>(ty: &'a Type, name: &str) -> Option<&'a Type> {
    let Type::Path(p) = ty else { return None };
    let last = p.path.segments.last()?;
    if last.ident != name {
        return None;
    }
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };
    args.args.iter().find_map(|arg| match arg {
        GenericArgument::Type(t) => Some(t),
        _ => None,
    })
}

pub(crate) fn source(
    container: &FieldContainer,
    generics: &Generics,
    root: &dyn ToTokens,
) -> Vec<TokenStream> {
    let Some(source) = container.selector_kind.source_field() else {
        return vec![];
    };
    let mut ty = source.transformation.target_ty();
    if container.selector_kind.is_whatever() {
        // Whatever stores Option<S>; source() formats the contained S.
        ty = inner_type(ty, "Option").unwrap_or(ty);
    }
    if !depends_on_generics(ty, generics) {
        return vec![];
    }
    // Preserve method-call autoderef for the standard source wrappers.
    loop {
        match ty {
            Type::Reference(r) => ty = &r.elem,
            _ => match inner_type(ty, "Box")
                .or_else(|| inner_type(ty, "Arc"))
                .or_else(|| inner_type(ty, "Rc"))
            {
                Some(inner) => ty = inner,
                None => break,
            },
        }
    }
    vec![quote!(#ty: #root::AsErrorSource)]
}

pub(crate) fn compat(
    container: &FieldContainer,
    generics: &Generics,
    root: &dyn ToTokens,
) -> Vec<TokenStream> {
    let mut result = vec![];
    if let Some(s) = container
        .selector_kind
        .source_field()
        .filter(|s| s.backtrace_delegate)
    {
        let ty = s.transformation.target_ty();
        if depends_on_generics(ty, generics) {
            result.push(quote!(#ty: #root::ErrorCompat));
        }
    }
    if let Some(f) = &container.backtrace_field {
        let ty = &f.ty;
        if depends_on_generics(ty, generics) {
            result.push(quote!(#ty: #root::AsBacktrace));
        }
    }
    result
}

pub(crate) fn construction(
    container: &FieldContainer,
    generics: &Generics,
    root: &dyn ToTokens,
) -> Vec<TokenStream> {
    let mut result = vec![];
    let fields = container
        .implicit_fields
        .iter()
        .chain(container.backtrace_field.iter());
    for field in fields {
        let ty = &field.ty;
        if depends_on_generics(ty, generics) {
            result.push(quote!(#ty: #root::GenerateImplicitData));
        }
    }
    // generate_with_source really consumes a dyn Error. Keep that requirement
    // only for constructors that request implicit data; do not discard its source.
    if !container.implicit_fields.is_empty() || container.backtrace_field.is_some() {
        result.extend(source(container, generics, root));
    }
    result
}

pub(crate) fn display<'a>(
    containers: impl IntoIterator<Item = &'a FieldContainer>,
    generics: &Generics,
    explicit: Option<&[WherePredicate]>,
) -> syn::Result<Vec<TokenStream>> {
    if let Some(explicit) = explicit {
        return Ok(explicit.iter().map(|p| quote!(#p)).collect());
    }
    let mut bounds = BTreeMap::new();
    for c in containers {
        if c.is_transparent {
            let ty = c
                .selector_kind
                .source_field()
                .unwrap()
                .transformation
                .target_ty();
            if depends_on_generics(ty, generics) {
                let b = quote!(#ty: ::core::fmt::Display);
                bounds.insert(b.to_string(), b);
            }
            continue;
        }
        let mut fields = BTreeMap::new();
        for f in c
            .user_fields()
            .iter()
            .chain(c.implicit_fields.iter())
            .chain(c.backtrace_field.iter())
            .chain(c.selector_kind.message_field())
        {
            fields.insert(f.name.to_string(), &f.ty);
        }
        if let Some(s) = c.selector_kind.source_field() {
            fields.insert(s.name.to_string(), s.transformation.target_ty());
        }
        // Concrete errors need no new inference, and retain existing diagnostics.
        if !fields.values().any(|ty| depends_on_generics(ty, generics)) {
            continue;
        }
        let mut positional = vec![];
        let mut named = BTreeMap::new();
        let (format, span) = if let Some(d) = &c.display_format {
            let Some(Expr::Lit(l)) = d.exprs.first() else {
                return Err(syn::Error::new_spanned(&c.name, "cannot infer display bounds from a non-literal format string; specify #[snafu(display_bounds(...))]"));
            };
            let Lit::Str(s) = &l.lit else { continue };
            for arg in d.exprs.iter().skip(1) {
                if let Expr::Assign(a) = arg {
                    if let Expr::Path(p) = &*a.left {
                        if let Some(id) = p.path.get_ident() {
                            named.insert(id.to_string(), &*a.right);
                        }
                    }
                } else {
                    positional.push(arg);
                }
            }
            (s.value(), s.span())
        } else if let Some(d) = &c.doc_comment {
            (d.content.clone(), c.name.span())
        } else {
            continue;
        };
        let uses = parse_format(&format).map_err(|_| syn::Error::new(span, "cannot infer display bounds from this format string; specify #[snafu(display_bounds(...))]"))?;
        for (argument, format_trait) in uses {
            let field_name = match argument {
                Argument::Position(i) => positional.get(i).and_then(|e| direct_field(e)),
                Argument::Name(n) => match named.get(&n) {
                    Some(e) => direct_field(e),
                    None => Some(n),
                },
            };
            let Some(ty) = field_name.as_ref().and_then(|n| fields.get(n)) else {
                continue;
            };
            if !depends_on_generics(ty, generics) {
                continue;
            }
            // Match arms bind fields by reference; Pointer formats that reference
            // rather than requiring the referent to implement Pointer.
            if format_trait == "Pointer" {
                continue;
            }
            let trait_name = syn::Ident::new(format_trait, span);
            let b = quote!(#ty: ::core::fmt::#trait_name);
            bounds.insert(b.to_string(), b);
        }
    }
    Ok(bounds.into_values().collect())
}

fn direct_field(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(p) => p.path.get_ident().map(ToString::to_string),
        Expr::Paren(p) => direct_field(&p.expr),
        Expr::Group(g) => direct_field(&g.expr),
        _ => None,
    }
}

#[derive(Debug, PartialEq)]
enum Argument {
    Position(usize),
    Name(String),
}

fn argument(s: &str) -> Result<Argument, ()> {
    if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        return s.parse().map(Argument::Position).map_err(|_| ());
    }
    // Rust format captures accept keywords as well as identifiers, but not `_`.
    use syn::ext::IdentExt;
    use syn::parse::Parser;
    syn::Ident::parse_any.parse_str(s).map_err(|_| ())?;
    if s == "_" || s.starts_with("r#") {
        return Err(());
    }
    Ok(Argument::Name(s.to_owned()))
}

/// Extract formatting-trait uses while preserving Rust's positional cursor.
/// Actual formatting and final validation still belong to write!.
fn parse_format(s: &str) -> Result<Vec<(Argument, &'static str)>, ()> {
    let mut chars = s.chars().peekable();
    let mut next = 0;
    let mut result = vec![];
    while let Some(c) = chars.next() {
        if c != '{' && c != '}' {
            continue;
        }
        if chars.peek() == Some(&c) {
            chars.next();
            continue;
        }
        if c == '}' {
            return Err(());
        }
        let mut contents = String::new();
        loop {
            match chars.next() {
                Some('}') => break,
                Some(c) => contents.push(c),
                None => return Err(()),
            }
        }
        let (arg, spec) = contents.split_once(':').unwrap_or((&contents, ""));
        let spec = spec.trim_end_matches(char::is_whitespace);
        let mut rest = spec;
        let mut spec_chars = rest.chars();
        let first = spec_chars.next();
        let second = spec_chars.next();
        if matches!(second, Some('<' | '^' | '>')) {
            rest = &rest[first.unwrap().len_utf8() + 1..];
        } else if matches!(first, Some('<' | '^' | '>')) {
            rest = &rest[1..];
        }
        if rest.starts_with(['+', '-']) {
            rest = &rest[1..];
        }
        if rest.starts_with('#') {
            rest = &rest[1..];
        }
        if rest.starts_with('0') && !rest.starts_with("0$") {
            rest = &rest[1..];
        }
        rest = consume_count(rest)?;
        if let Some(precision) = rest.strip_prefix('.') {
            if let Some(r) = precision.strip_prefix('*') {
                next += 1;
                rest = r;
            } else {
                rest = consume_count(precision)?;
                if rest.len() == precision.len() {
                    return Err(());
                }
            }
        }
        let format_trait = match rest {
            "" => "Display",
            "?" | "x?" | "X?" => "Debug",
            "x" => "LowerHex",
            "X" => "UpperHex",
            "o" => "Octal",
            "b" => "Binary",
            "p" => "Pointer",
            "e" => "LowerExp",
            "E" => "UpperExp",
            _ => return Err(()),
        };
        let arg = arg.trim_end_matches(char::is_whitespace);
        let arg = if arg.is_empty() {
            let i = next;
            next += 1;
            Argument::Position(i)
        } else {
            argument(arg)?
        };
        result.push((arg, format_trait));
    }
    Ok(result)
}

fn consume_count(s: &str) -> Result<&str, ()> {
    if let Some(i) = s.split('.').next().unwrap_or(s).find('$') {
        // A count is an index/name followed by $, never a formatting flag.
        argument(&s[..i])?;
        return Ok(&s[i + 1..]);
    }
    Ok(s.trim_start_matches(|c: char| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn format_arguments_and_flags() {
        assert_eq!(
            parse_format("{{}} {0} {0:#?} {name:🦀^+#018.2x} {0:x?}").unwrap(),
            vec![
                (Argument::Position(0), "Display"),
                (Argument::Position(0), "Debug"),
                (Argument::Name("name".into()), "LowerHex"),
                (Argument::Position(0), "Debug"),
            ]
        );
        assert_eq!(
            parse_format("{:.*} {} {2:.width$E}").unwrap(),
            vec![
                (Argument::Position(1), "Display"),
                (Argument::Position(2), "Display"),
                (Argument::Position(2), "UpperExp"),
            ]
        );
    }
}
