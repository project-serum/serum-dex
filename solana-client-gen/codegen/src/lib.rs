//! codegen defines the proc macro re-exported by solana_client_gen.
//! For documentation, see the main crate.

use heck::SnakeCase;
use proc_quote::quote;
use syn::parse_macro_input;

#[proc_macro_attribute]
pub fn solana_client_gen(
    args: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    // The one and only argument of the macro should be the boolean marker for
    // using the client extension
    let needs_client_ext = parse_client_ext_arg(args);

    // Interpet token stream as the instruction `mod`.
    let instruction_mod = parse_macro_input!(input as syn::ItemMod);

    // Pull out the enum within the instruction mod.
    let instruction_enum_item: syn::ItemEnum = {
        let all_enums = instruction_mod
            .content
            .as_ref()
            .expect("content is None")
            .1
            .iter()
            .filter_map(|item| match item {
                syn::Item::Enum(e) => Some(syn::Item::Enum(e.clone())),
                _ => None,
            })
            .collect::<Vec<syn::Item>>();
        match all_enums.first().unwrap().clone() {
            syn::Item::Enum(e) => e,
            _ => panic!("must have an instruction enum"),
        }
    };

    // Parse the instruction enum and generate code from each enum variant.
    let (client_methods, instruction_methods) = enum_to_methods(&instruction_enum_item);

    // Now recreate the highest level instruction `mod`, but with our new
    // instruction_methods inside.
    let new_instruction_mod = {
        let mod_ident = instruction_mod.ident;

        quote! {
            pub mod #mod_ident {
                use super::*;

                #instruction_methods

                #instruction_enum_item
            }
        }
    };

    let client = quote! {
        use super::*;
        use solana_client_gen::solana_sdk::instruction::InstructionError;
        use solana_client_gen::solana_sdk::transaction::TransactionError;
        use solana_client_gen::solana_client::client_error::ClientErrorKind as RpcClientErrorKind;

        #[derive(Debug, Error)]
        pub enum ClientError {
            #[error("Invalid keypair filename: {0}")]
            InvalidKeyPairFile(String),
            #[error("Error invoking rpc: {0}")]
            RpcError(#[from] solana_client::client_error::ClientError),
            #[error("{0}")]
            RawError(String),
        }

        impl ClientError {
            // error_code returns Some(error_code) returned by the on chain program
            // and none if the error resulted from somewhere else.
            //
            // TODO: there's gotta be a cleaner way of unpacking this.
            pub fn error_code(&self) -> Option<u32> {
                match self {
                    ClientError::RpcError(e) => match e.kind() {
                        RpcClientErrorKind::TransactionError(e) => match e {
                            TransactionError::InstructionError(_, instr_error) => match instr_error {
                                InstructionError::Custom(error_code) => {
                                    Some(*error_code)
                                }
                                _ => None,
                            },
                            _ => None,
                        },
                        _ => None,
                    },
                    _ => None,
                }
            }
        }

        // Client is the RPC client generated to talk to a program running
        // on a configured Solana cluster.
        pub struct Client {
            program_id: Pubkey,
            payer: Keypair,
            rpc: RpcClient,
            opts: RequestOptions,
            url: String,
        }

        impl Client {
            pub fn new(
                program_id: Pubkey,
                payer: Keypair,
                url: &str,
                given_opts: Option<RequestOptions>,
            ) -> Self {
                let rpc = RpcClient::new(url.to_string());
                let opts = match given_opts {
                    Some(opts) => opts,
                    // Use these default options if None are given.
                    None => RequestOptions {
                        commitment: CommitmentConfig::single(),
                        tx: RpcSendTransactionConfig {
                            skip_preflight: true,
                            ..RpcSendTransactionConfig::default()
                        },
                    },
                };
                Self {
                    program_id,
                    payer,
                    rpc,
                    opts,
                    url: url.to_string(),
                }
            }

            pub fn from_keypair_file(program_id: Pubkey, filename: &str, url: &str) -> Result<Self, ClientError> {
                let kp = solana_sdk::signature::read_keypair_file(filename)
                    .map_err(|_| ClientError::InvalidKeyPairFile(filename.to_string()))?;
                Ok(Self::new(program_id, kp, url, None))
            }

            // Builder method to set the default options for each RPC request.
            pub fn with_options(mut self, opts: RequestOptions) -> Self {
                self.opts = opts;
                self
            }

            pub fn rpc(&self) -> &RpcClient {
                &self.rpc
            }

            pub fn payer(&self) -> &Keypair {
                &self.payer
            }

            pub fn program(&self) -> &Pubkey {
                &self.program_id
            }

            pub fn options(&self) -> &RequestOptions {
                &self.opts
            }

            pub fn url(&self) -> &str {
                &self.url
            }

            #client_methods
        }

        // Used for tests.
        impl solana_client_gen::prelude::ClientGen for Client {
            fn from_keypair_file(
                program_id: Pubkey,
                filename: &str,
                url: &str,
            ) -> anyhow::Result<Client> {
                Client::from_keypair_file(program_id, filename, url)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))
            }
            fn with_options(self, opts: solana_client_gen::prelude::RequestOptions) -> Client {
                self.with_options(opts)
            }
            fn rpc(&self) -> &RpcClient {
                self.rpc()
            }
            fn payer(&self) -> &Keypair {
                self.payer()
            }
            fn program(&self) -> &Pubkey {
                self.program()
            }
        }
    };

    // Generate the entire client module.
    let client_mod = quote! {
        #[cfg(feature = "client")]
        pub mod client {
            #client
        }
    };

    // Now put it all together.
    //
    // There are two options: with or without the client-extension.

    // By default, just output the new modules directly.
    let default_output = quote! {
        #client_mod
        #new_instruction_mod
    };

    // Instead, if the client-extension is enabled, we output yet another
    // macro.
    //
    // When this macro is enabled, a client can extend the client
    // to add custom apis. For example.
    //
    // solana_client_gen_extension! {
    //   impl Client {
    //     my_custom_api() {
    //       ...
    //     }
    //   }
    // }
    let client_ext_macro = quote! {
        #[macro_export]
        macro_rules! solana_client_gen_extension {
            ($($client_ext:tt)*) => {
                pub mod client {
                    #client
                    $($client_ext)*
                }
                #new_instruction_mod
            }
        }
    };

    proc_macro::TokenStream::from(match needs_client_ext {
        true => client_ext_macro,
        false => default_output,
    })
}

// Parses the instruction enum inside the given instruction module and coverts
// each variant into several token streams:
//
// * Client RPC methods for each instruction variant.
// * Instruction methods for generating instances of solana_sdk::instruction::Instruction.
// * Decode and dispatch tree, i.e., the code to execute on entry to the program.
//
fn enum_to_methods(
    instruction_enum: &syn::ItemEnum,
) -> (proc_macro2::TokenStream, proc_macro2::TokenStream) {
    // The name of the enum defining the program instruction.
    let instruction_enum_ident = instruction_enum.ident.clone();

    // Parse the enum and create methods.
    let (variant_client_methods, variant_instruction_methods): (
        Vec<proc_macro2::TokenStream>,
        Vec<proc_macro2::TokenStream>,
    ) = instruction_enum
        .variants
        .iter()
        .map(|variant| {
            // The name of the enum variant (i.e., the instruction name).
            let variant_name = &variant.ident;

            // Translate the name into snake_case for creating methods.
            let method_name = proc_macro2::Ident::new(
                &variant_name.to_string().to_snake_case(),
                proc_macro2::Span::call_site(),
            );

            // For each enum variant, parse both the method_args, i.e.,
            // the `arg-name: type`, and the arg idents--i.e., the `arg-name`.
            let (method_args_vec, method_arg_idents_vec): (
                Vec<proc_macro2::TokenStream>,
                Vec<proc_macro2::TokenStream>,
            ) = match &variant.fields {
                syn::Fields::Named(fields) => fields
                    .named
                    .iter()
                    .map(|field| {
                        let field_ident =
                            field.ident.clone().expect("field identifier not found");
                        let field_ty = field.ty.clone();
                        let method_arg = quote! {
                            #field_ident: #field_ty
                        };
                        let method_arg_ident = quote! {
                            #field_ident
                        };
                        (method_arg, method_arg_ident)
                    })
                    .unzip(),
                syn::Fields::Unit =>  (vec![], vec![]),
                syn::Fields::Unnamed(_fields) => panic!("Unamed variants not supported, yet"),
            };

            // All method args with identifiers and types, e.g., `my_arg: u64`.
            let method_args = quote! {
                #(#method_args_vec),*
            };

            // All method args, without types, e.g., `my_arg`.
            let method_arg_idents = quote! {
                #(#method_arg_idents_vec),*
            };


            // The instruction enum, with var names but no types.
            let instruction_enum = {
                if variant.fields == syn::Fields::Unit {
                    quote! {
                        #instruction_enum_ident::#variant_name
                    }
                } else {
                    quote! {
                        #instruction_enum_ident::#variant_name {
                            #method_arg_idents,
                        }
                    }
                }
            };
            // Generate the method to create a Solana `Instruction` representing this
            // enum variant.
            let instruction_method = {
                quote! {
                    pub fn #method_name(program_id: Pubkey, accounts: &[AccountMeta], #method_args) -> Instruction {
                        // Create the instruction enum.
                        let instruction = #instruction_enum;

                        // Serialize.
                        let size = instruction
                            .size()
                            .expect("instructions must be serializable")
                            as usize;
                        let mut data = vec![0u8; size];
                        #instruction_enum_ident::pack(instruction, &mut data)
                            .expect("instruction must be serializable");
                        Instruction {
                            program_id,
                            data,
                            accounts: accounts.to_vec(),
                        }
                    }
                }
            };

            let method_name_with_signers = proc_macro2::Ident::new(
                format!("{}_with_signers", variant_name.to_string().to_snake_case()).as_str(),
                proc_macro2::Span::call_site(),
            );
            // Generate the high level client method to make an RPC with this
            // instruction.
            let client_method = quote! {
                // Invokes the rpc with the client's payer as the only signer.
                pub fn #method_name(&self, accounts: &[AccountMeta], #method_args) -> Result<Signature, ClientError> {
                    self.#method_name_with_signers(&[&self.payer], accounts, #method_arg_idents)
                }
                // Invokes the rpc with the given signers.
                //
                // Make sure to add the payer configured on the client to the list
                // of signers if you're to use this method.
                pub fn #method_name_with_signers<T: Signers>(&self, signers: &T, accounts: &[AccountMeta], #method_args) -> Result<Signature, ClientError> {
                    let instructions = vec![
                        super::instruction::#method_name(
                            self.program_id,
                            accounts,
                            #method_arg_idents
                        ),
                    ];
                    let (recent_hash, _fee_calc) = self
                        .rpc
                        .get_recent_blockhash()
                        .map_err(ClientError::RpcError)?;
                    let txn = Transaction::new_signed_with_payer(
                        &instructions,
                        Some(&self.payer.pubkey()),
                        signers,
                        recent_hash,
                    );
                    self
                        .rpc
                        .send_and_confirm_transaction_with_spinner_and_config(
                            &txn,
                            self.opts.commitment,
                            self.opts.tx,
                        )
                        .map_err(ClientError::RpcError)
                }
            };

            (client_method, instruction_method)
        })
        .unzip();

    // The token stream of all generated rpc client methods.
    let client_methods = quote! {
        #(#variant_client_methods)*
    };
    // The token stream of all generated `solana_sdk::instruction::Instruction`
    // generation method.
    let instruction_methods = quote! {
        #(#variant_instruction_methods)*
    };

    (client_methods, instruction_methods)
}

fn parse_client_ext_arg(args: proc_macro::TokenStream) -> bool {
    match args.to_string().as_ref() {
        "ext" => true,
        _ => false,
    }
}
