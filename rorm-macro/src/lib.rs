//! This crate tries to follow the base layout proposed by a [ferrous-systems.com](https://ferrous-systems.com/blog/testing-proc-macros/#the-pipeline) blog post.
extern crate proc_macro;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;

#[proc_macro_derive(DbEnum)]
pub fn derive_db_enum(input: TokenStream) -> TokenStream {
    rorm_macro_impl::derive_db_enum(input.into()).into()
}

#[proc_macro_derive(Model, attributes(rorm))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    rorm_macro_impl::derive_model(input.into()).into()
}

#[proc_macro_derive(Patch, attributes(rorm))]
pub fn derive_patch(input: TokenStream) -> TokenStream {
    rorm_macro_impl::derive_patch(input.into()).into()
}

#[proc_macro_attribute]
pub fn rorm_main(args: TokenStream, item: TokenStream) -> TokenStream {
    let main = syn::parse_macro_input!(item as syn::ItemFn);
    let feature = syn::parse::<syn::LitStr>(args)
        .unwrap_or_else(|_| syn::LitStr::new("rorm-main", Span::call_site()));

    (if main.sig.ident == "main" {
        quote! {
            #[cfg(feature = #feature)]
            fn main() -> Result<(), String> {
                let mut file = ::std::fs::File::create(".models.json").map_err(|err| err.to_string())?;
                ::rorm::write_models(&mut file)?;
                return Ok(());
            }
            #[cfg(not(feature = #feature))]
            #main
        }
    } else {
        quote! {
            compile_error!("only allowed on main function");
            #main
        }
    }).into()
}

/// ```ignored
/// impl_tuple!(some_macro, 2..5);
///
/// // produces
///
/// some_macro!(0: T0, 1: T1);               // tuple of length 2
/// some_macro!(0: T0, 1: T1, 2: T2);        // tuple of length 3
/// some_macro!(0: T0, 1: T1, 2: T2, 3: T3); // tuple of length 4
/// ```
#[proc_macro]
pub fn impl_tuple(args: TokenStream) -> TokenStream {
    // handwritten without dependencies just for fun
    use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenTree as TT};

    let args = Vec::from_iter(args);
    let [TT::Ident(macro_ident), TT::Punct(comma), TT::Literal(start), TT::Punct(fst_dot), TT::Punct(snd_dot), TT::Literal(end)] =
        &args[..]
    else {
        panic!()
    };
    if *comma != ','
        || *fst_dot != '.'
        || *snd_dot != '.' && matches!(fst_dot.spacing(), Spacing::Alone)
    {
        panic!();
    }

    let start: usize = start.to_string().parse().unwrap();
    let end: usize = end.to_string().parse().unwrap();

    let mut tokens = TokenStream::default();
    for until in start..end {
        let mut impl_args = TokenStream::new();
        for index in 0..until {
            impl_args.extend([
                TT::Literal(Literal::usize_unsuffixed(index)),
                TT::Punct(Punct::new(':', Spacing::Alone)),
                TT::Ident(Ident::new(&format!("T{index}"), Span::call_site())),
                TT::Punct(Punct::new(',', Spacing::Alone)),
            ]);
        }
        tokens.extend([
            TT::Ident(macro_ident.clone()),
            TT::Punct(Punct::new('!', Spacing::Alone)),
            TT::Group(Group::new(Delimiter::Parenthesis, impl_args)),
            TT::Punct(Punct::new(';', Spacing::Alone)),
        ]);
    }
    tokens
}
