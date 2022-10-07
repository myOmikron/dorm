use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::{format_ident, quote};

use crate::Errors;

pub fn impl_anno_builder(args: TokenStream) -> TokenStream {
    let errors = Errors::new();

    // Collect args into vec in order to call `.chunks_exact`
    let args: Vec<TokenTree> = args.into_iter().collect();

    // Unpack the args vec
    let mut fields = Vec::new();
    let mut types = Vec::new();
    for slice in args.chunks_exact(3) {
        match slice {
            [TokenTree::Ident(field_name), TokenTree::Ident(type_name), TokenTree::Punct(punct)]
                if punct.as_char() == ',' =>
            {
                types.push(type_name);
                fields.push(field_name);
            }
            [a, _, b] => {
                errors.push_new_spanned(a.span(), b.span(), "wrong token types");
            }
            _ => unreachable!("`chunks_exact` only yields three element slices"),
        }
    }

    // Build code
    let alphabet = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J']
        .map(|character| Ident::new(character.encode_utf8(&mut [0; 4]), Span::call_site()));
    let mut steps = Vec::new();
    for index in 0..types.len() {
        let pre_alphabet = &alphabet[0..index];
        let post_alphabet = &alphabet[(index + 1)..types.len()];
        let pre_types = &types[0..index];
        let post_types = &types[(index + 1)..types.len()];
        let pre_fields = &fields[0..index];
        let post_fields = &fields[(index + 1)..types.len()];
        let ty = types[index];
        let field = fields[index];
        let implicit = format_ident!("implicit_{}", field);
        let forbidden = format_ident!("forbidden_{}", field);
        steps.push(quote! {
            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*> Step<#ty> for
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                type Output = Annotations<#(#pre_alphabet,)* #ty, #(#post_alphabet,)*>;
            }
            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*>
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                pub const fn #field(self, #field: #ty) -> <Self as Step<#ty>>::Output {
                    let Annotations {#(#pre_fields,)* #(#post_fields,)* ..} = self;
                    Annotations {#(#fields,)*}
                }
            }

            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*> Step<Implicit<#ty>> for
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                type Output = Annotations<#(#pre_alphabet,)* Implicit<#ty>, #(#post_alphabet,)*>;
            }
            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*>
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                pub const fn #implicit(self, #field: #ty) -> <Self as Step<Implicit<#ty>>>::Output {
                    let #field = Implicit::new(#field);
                    let Annotations {#(#pre_fields,)* #(#post_fields,)* ..} = self;
                    Annotations {#(#fields,)*}
                }
            }

            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*> Step<Forbidden<#ty>> for
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                type Output = Annotations<#(#pre_alphabet,)* Forbidden<#ty>, #(#post_alphabet,)*>;
            }
            impl<#(#pre_alphabet: Annotation<#pre_types>,)* #(#post_alphabet: Annotation<#post_types>,)*>
            Annotations<#(#pre_alphabet,)* NotSet<#ty>, #(#post_alphabet,)*> {
                pub const fn #forbidden(self) -> <Self as Step<Forbidden<#ty>>>::Output {
                    let #field = Forbidden::new();
                    let Annotations {#(#pre_fields,)* #(#post_fields,)* ..} = self;
                    Annotations {#(#fields,)*}
                }
            }
        });
    }

    quote! {
        mod _internal {
            use rorm_declaration::hmr::annotations::*;

            /// Generic struct storing a [`Field`]'s annotations.
            ///
            /// While this is its primary annotations,
            /// this struct also implements something similar to the builder pattern.
            /// This enables the [`Model`] macro to extend a "default" [`Annotations`] struct
            /// with the concrete annotations set by the user.
            /// These defaults are defined in [`AsDbType`]
            /// and should contain [`Implicit`] or [`Forbidden`] annotations (if any).
            ///
            /// [`AsDbType`]: crate::model::AsDbType
            /// [`Field`]: crate::model::Field
            /// [`Model`]: rorm_macro::Model
            pub struct Annotations<#(#alphabet: Annotation<#types>),*> {
                #(
                    pub #fields: #alphabet,
                )*
            }

            #(#steps)*

            pub type NotSetAnnotations = Annotations<#(NotSet<#types>),*>;
            impl NotSetAnnotations {
                pub const fn new() -> Self {
                    Annotations {
                        #(
                            #fields: NotSet::new(),
                        )*
                    }
                }
            }

            impl<#(#alphabet: Annotation<#types>),*> AsImr for Annotations<#(#alphabet),*> {
                type Imr = Vec<rorm_declaration::imr::Annotation>;

                fn as_imr(&self) -> Self::Imr {
                    let mut annos = Vec::new();
                    #(
                        if let Some(anno) = self.#fields.as_imr() {
                            annos.push(anno);
                        }
                    )*
                    annos
                }
            }

            #errors
        }
        pub use _internal::*;
    }
}
