use crate::helpers::{get_derive_attributes, StructInfo};
use crate::net::packets::get_packet_ids_from_attributes;
use crate::static_loading::packets::PacketBoundiness;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Fields};

/// Writes the packet id for the version being encoded for. Ids move between versions, and a packet
/// can be absent from an older one entirely, so the table is baked in and indexed at run time.
fn generate_packet_id_snippets(
    ids: Option<(String, Vec<Option<i32>>)>,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let Some((packet_name, ids)) = ids else {
        return (quote! {}, quote! {});
    };

    let entries = ids.iter().map(|id| match id {
        Some(id) => quote! { Some(#id) },
        None => quote! { None },
    });
    let count = ids.len();

    let lookup = quote! {
        const PACKET_IDS: [Option<i32>; #count] = [#(#entries),*];
        let Some(packet_id) = PACKET_IDS[opts.version.index()] else {
            return Err(ferrumc_net_codec::encode::errors::NetEncodeError::PacketNotInVersion {
                packet: #packet_name,
                version: opts.version,
            });
        };
        let packet_id = ferrumc_net_codec::net_types::var_int::VarInt::new(packet_id);
    };

    let sync_snippet = quote! {
        #lookup
        <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode(&packet_id, writer, &opts.nested())?;
    };

    let async_snippet = quote! {
        #lookup
        <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode_async(&packet_id, writer, &opts.nested()).await?;
    };

    (sync_snippet, async_snippet)
}

// Generate field encoding expressions for structs
/// The version a field was added in, from `#[since(V26_2)]`. A field without one is written for
/// every version.
fn field_since(field: &syn::Field) -> Option<syn::Ident> {
    field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("since"))
        .map(|attr| {
            attr.parse_args::<syn::Ident>()
                .expect("#[since(..)] takes one ProtocolVersion variant, e.g. #[since(V26_2)]")
        })
}

/// Wraps a field's encoding in a version check when the field only exists from some release on.
fn gate_on_version(
    since: Option<syn::Ident>,
    body: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match since {
        Some(version) => quote! {
            if opts.version >= ferrumc_net_codec::version::ProtocolVersion::#version {
                #body
            }
        },
        None => body,
    }
}

fn generate_field_encoders(fields: &syn::Fields) -> proc_macro2::TokenStream {
    let encode_fields = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        gate_on_version(field_since(field), quote! {
            <#field_ty as ferrumc_net_codec::encode::NetEncode>::encode(&self.#field_name, writer, &opts.nested())?;
        })
    });
    quote! { #(#encode_fields)* }
}

fn generate_async_field_encoders(fields: &syn::Fields) -> proc_macro2::TokenStream {
    let encode_fields = fields.iter().map(|field| {
        let field_name = field.ident.as_ref().unwrap();
        let field_ty = &field.ty;
        gate_on_version(field_since(field), quote! {
            <#field_ty as ferrumc_net_codec::encode::NetEncode>::encode_async(&self.#field_name, writer, &opts.nested()).await?;
        })
    });
    quote! { #(#encode_fields)* }
}

// Generate enum variant encoding using static dispatch
fn generate_enum_encoders(
    data: &syn::DataEnum,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    let variants = data.variants.iter().map(|variant| {
        let variant_ident = &variant.ident;

        match &variant.fields {
            Fields::Named(fields) => {
                let field_idents: Vec<_> = fields.named.iter()
                    .map(|f| f.ident.as_ref().unwrap())
                    .collect();
                let field_tys: Vec<_> = fields.named.iter()
                    .map(|f| &f.ty)
                    .collect();

                (quote! {
                    Self::#variant_ident { #(#field_idents),* } => {
                        #(
                            <#field_tys as ferrumc_net_codec::encode::NetEncode>::encode(#field_idents, writer, &opts.nested())?;
                        )*
                    }
                },
                 quote! {
                    Self::#variant_ident { #(#field_idents),* } => {
                        #(
                            <#field_tys as ferrumc_net_codec::encode::NetEncode>::encode_async(#field_idents, writer, &opts.nested()).await?;
                        )*
                    }
                })
            }
            Fields::Unnamed(fields) => {
                let field_names: Vec<_> = (0..fields.unnamed.len())
                    .map(|i| syn::Ident::new(&format!("field{i}"), proc_macro2::Span::call_site()))
                    .collect();
                let field_tys: Vec<_> = fields.unnamed.iter()
                    .map(|f| &f.ty)
                    .collect();

                (quote! {
                    Self::#variant_ident(#(#field_names),*) => {
                        #(
                            <#field_tys as ferrumc_net_codec::encode::NetEncode>::encode(#field_names, writer, &opts.nested())?;
                        )*
                    }
                },
                 quote! {
                    Self::#variant_ident(#(#field_names),*) => {
                        #(
                            <#field_tys as ferrumc_net_codec::encode::NetEncode>::encode_async(#field_names, writer, &opts.nested()).await?;
                        )*
                    }
                })
            }
            Fields::Unit => (
                quote! {
                    Self::#variant_ident => {}
                },
                quote! {
                    Self::#variant_ident => {}
                }
            ),
        }
    }).unzip::<_, _, Vec<_>, Vec<_>>();

    let (sync_variants, async_variants) = variants;

    (
        quote! {
            match self {
                #(#sync_variants)*
            }
        },
        quote! {
            match self {
                #(#async_variants)*
            }
        },
    )
}

pub(crate) fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let packet_attr = get_derive_attributes(&input, "packet");
    let (packet_id_snippet, async_packet_id_snippet) = generate_packet_id_snippets(
        get_packet_ids_from_attributes(packet_attr.as_slice(), &PacketBoundiness::Clientbound),
    );

    let (sync_impl, async_impl) = match &input.data {
        syn::Data::Struct(data) => {
            let field_encoders = generate_field_encoders(&data.fields);
            let async_field_encoders = generate_async_field_encoders(&data.fields);

            (
                quote! {
                    fn encode<W: std::io::Write>(&self, writer: &mut W, opts: &ferrumc_net_codec::encode::NetEncodeOpts) -> Result<(),  ferrumc_net_codec::encode::errors::NetEncodeError> {
                        match opts.framing {
                            ferrumc_net_codec::encode::Framing::None => {
                                #packet_id_snippet
                                #field_encoders
                            }
                            ferrumc_net_codec::encode::Framing::WithLength => {
                                let actual_writer = writer;
                                let mut writer = Vec::new();
                                let mut writer = &mut writer;

                                #packet_id_snippet
                                #field_encoders

                                let len: ferrumc_net_codec::net_types::var_int::VarInt = writer.len().into();
                                <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode(&len, actual_writer, &opts.nested())?;
                                actual_writer.write_all(writer)?;
                            }
                            e => unimplemented!("Unsupported option for NetEncode: {:?}", e),
                        }
                        Ok(())
                    }
                },
                quote! {
                    async fn encode_async<W: tokio::io::AsyncWrite + std::marker::Unpin>(&self, writer: &mut W, opts: &ferrumc_net_codec::encode::NetEncodeOpts) -> Result<(),  ferrumc_net_codec::encode::errors::NetEncodeError> {
                        match opts.framing {
                            ferrumc_net_codec::encode::Framing::None => {
                                #async_packet_id_snippet
                                #async_field_encoders
                            }
                            ferrumc_net_codec::encode::Framing::WithLength => {
                                let actual_writer = writer;
                                let mut writer = Vec::new();
                                let mut writer = &mut writer;

                                #async_packet_id_snippet
                                #field_encoders

                                let len: ferrumc_net_codec::net_types::var_int::VarInt = writer.len().into();
                                <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode_async(&len, actual_writer, &opts.nested()).await?;
                                <W as tokio::io::AsyncWriteExt>::write_all(actual_writer, writer).await?;
                            }
                            e => unimplemented!("Unsupported option for NetEncode: {:?}", e),
                        }
                        Ok(())
                    }
                },
            )
        }
        syn::Data::Enum(data) => {
            let (sync_enum_encoder, async_enum_encoder) = generate_enum_encoders(data);

            (
                quote! {
                    fn encode<W: std::io::Write>(&self, writer: &mut W, opts: &ferrumc_net_codec::encode::NetEncodeOpts) -> Result<(),  ferrumc_net_codec::encode::errors::NetEncodeError> {
                        match opts.framing {
                            ferrumc_net_codec::encode::Framing::None => {
                                #packet_id_snippet
                                #sync_enum_encoder
                            }
                            ferrumc_net_codec::encode::Framing::WithLength => {
                                let actual_writer = writer;
                                let mut writer = Vec::new();
                                let mut writer = &mut writer;

                                #packet_id_snippet
                                #sync_enum_encoder

                                let len: ferrumc_net_codec::net_types::var_int::VarInt = writer.len().into();
                                <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode(&len, actual_writer, &opts.nested())?;
                                actual_writer.write_all(writer)?;
                            }
                            e => unimplemented!("Unsupported option for NetEncode: {:?}", e),
                        }
                        Ok(())
                    }
                },
                quote! {
                    async fn encode_async<W: tokio::io::AsyncWrite + std::marker::Unpin>(&self, writer: &mut W, opts: &ferrumc_net_codec::encode::NetEncodeOpts) -> Result<(),  ferrumc_net_codec::encode::errors::NetEncodeError> {
                        match opts.framing {
                            ferrumc_net_codec::encode::Framing::None => {
                                #async_packet_id_snippet
                                #async_enum_encoder
                            }
                            ferrumc_net_codec::encode::Framing::WithLength => {
                                let actual_writer = writer;
                                let mut writer = Vec::new();
                                let mut writer = &mut writer;

                                #async_packet_id_snippet
                                #sync_enum_encoder

                                let len: ferrumc_net_codec::net_types::var_int::VarInt = writer.len().into();
                                <ferrumc_net_codec::net_types::var_int::VarInt as ferrumc_net_codec::encode::NetEncode>::encode_async(&len, actual_writer, &opts.nested()).await?;
                                <W as tokio::io::AsyncWriteExt>::write_all(actual_writer, writer).await?;
                            }
                            e => unimplemented!("Unsupported option for NetEncode: {:?}", e),
                        }
                        Ok(())
                    }
                },
            )
        }
        _ => unimplemented!("NetEncode can only be derived for structs and enums"),
    };

    let StructInfo {
        struct_name,
        impl_generics,
        ty_generics,
        where_clause,
        lifetime: _lifetime,
        ..
    } = crate::helpers::extract_struct_info(&input, None);

    TokenStream::from(quote! {
        impl #impl_generics ferrumc_net_codec::encode::NetEncode for #struct_name #ty_generics #where_clause {
            #sync_impl
            #async_impl
        }
    })
}
