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
    // The one and only argument of the macro should be the Coder struct.
    let coder_struct = match args.to_string().as_ref() {
        "" => None,
        _ => Some(parse_macro_input!(args as syn::Ident)),
    };

    // Interpet token stream as the instruction `mod`.
    let instruction_mod = parse_macro_input!(input as syn::ItemMod);

    // Parse the instruction enum inside the mod and translate to client methods.
    let (client_methods, instruction_methods, decode_and_dispatch_tree, coder_definition) =
        mod_to_methods(&instruction_mod, coder_struct);

    // The api_macro is a meta-macro emmited from this attribute macro.
    //
    // It is used to declare a Solana program, for example, in lib.rs
    // where `solana_sdk::entrypoint!` normally is, you would instead
    // write `solana_program_api!();`
    //
    // And in the same file, implement all your api methods, which
    // correspond to the instruction enum variants defined in the
    // interface.
    //
    // Note: this isn't enabled yet because Solana's bpf toolchain
    //       is on a version of rust that doesn't support proc-macros.
    //       Enable this once they upgrade.
    let _api_meta_macro = quote! {
        #[cfg(feature = "program")]
        #[macro_export]
        macro_rules! solana_program_api {
            () => {
                solana_sdk::entrypoint!(process_instruction);
                fn process_instruction(
                    program_id: &Pubkey,
                    accounts: &[AccountInfo],
                    instruction_data: &[u8],
                ) -> ProgramResult {
                    #decode_and_dispatch_tree
                }
            }
        }
    };

    // Now recreate the highest level instruction mod, but with our new
    // instruction_methods inside.
    let new_instruction_mod = {
        let mod_ident = instruction_mod.clone().ident;
        let enum_items = instruction_mod
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
        let instruction_enum = enum_items.first().clone().unwrap();

        quote! {
            pub mod #mod_ident {
                use super::*;

                #instruction_methods

                #instruction_enum
            }
        }
    };

    let client = quote! {
        #[cfg(feature = "client")]
        pub mod client {
            use super::*;

            #[derive(Debug, Error)]
            pub enum ClientError {
                #[error("Invalid keypair filename")]
                InvalidKeyPairFile(String),
                #[error("Error invoking rpc")]
                RpcError(solana_client::client_error::ClientError),
                #[error("Raw error")]
                RawError(String),
            }

            #[derive(Debug)]
            pub struct RequestOptions {
                pub commitment: CommitmentConfig,
                pub tx: RpcSendTransactionConfig,
            }

            // Client is the RPC client generated to talk to a program running
            // on a configured Solana cluster.
            pub struct Client {
                program_id: Pubkey,
                payer: Keypair,
                rpc: RpcClient,
                opts: RequestOptions,
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
                                preflight_commitment: None,
                            },
                        },
                    };
                    Self {
                        program_id,
                        payer,
                        rpc,
                        opts,
                    }
                }

                pub fn from_keypair_file(program_id: Pubkey, filename: &str, url: &str) -> Result<Self, ClientError> {
                    let kp = solana_sdk::signature::read_keypair_file(filename)
                        .map_err(|e| ClientError::InvalidKeyPairFile(filename.to_string()))?;
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

                #client_methods
            }
        }
    };

    // Output the transormed AST with the new client, new instruction mod,
    // and new coder definition.
    proc_macro::TokenStream::from(quote! {
        #client

        #new_instruction_mod

        #coder_definition
    })
}

// Parses the instruction enum inside the given instruction module and coverts
// each variant into several token streams:
//
// * Client RPC methods for each instruction variant.
// * Instruction methods for generating instances of solana_sdk::instruction::Instruction.
// * Decode and dispatch tree, i.e., the code to execute on entry to the program.
// * Coder struct for serialization.
//
fn mod_to_methods(
    instruction_mod: &syn::ItemMod,
    coder_struct_opt: Option<syn::Ident>,
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    let coder_struct = match &coder_struct_opt {
        None => quote! {
                _DefaultCoder
        },
        Some(cs) => quote! {
                #cs
        },
    };

    // When combined together, all the dispatch arms are used on
    // program entry, to define a `match` statement to interpret an
    // instruction variant, and dispatch the request to the program's
    // corresponding api method.
    let mut dispatch_arms = vec![];

    // The name of the enum defining the program instruction.
    //
    // Dummy initialization, here, so that we can do the actual initialization
    // inside the map below.
    let mut instruction_enum_ident =
        &proc_macro2::Ident::new("_dummy", proc_macro2::Span::call_site());

    // Parse the enum and create methods.
    let (client_methods, instruction_methods): (
        proc_macro2::TokenStream,
        proc_macro2::TokenStream,
    ) = instruction_mod
        .content
        .as_ref()
        .unwrap()
        .1
        .iter()
        .filter_map(|item| match item {
            syn::Item::Enum(instruction_enum) => {
                instruction_enum_ident = &instruction_enum.ident;
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
                                .filter_map(|field| {
                                    let field_ident =
                                        field.ident.clone().expect("field identifier not found");
                                    let field_ty = field.ty.clone();
                                    let method_arg = quote! {
                                            #field_ident: #field_ty
                                    };
                                    let method_arg_ident = quote! {
                                        #field_ident
                                    };
                                    Some((method_arg, method_arg_ident))
                                })
                                .unzip(),
                            syn::Fields::Unit =>  (vec![], vec![]),
                            syn::Fields::Unnamed(_fields) => panic!("Unamed variants not supported, yet"),
                        };

                        // All method args with identifiers and types, e.g., my_arg: u64.
                        let method_args = quote! {
                            #(#method_args_vec),*
                        };

                        // All method args, without types, e.g., my_arg.
                        let method_arg_idents = quote! {
                            #(#method_arg_idents_vec),*
                        };

                        // Generate the method to create a Solana `Instruction` representing this
                        // enum variant.
                        let instruction_method = quote! {
                            pub fn #method_name(program_id: Pubkey, accounts: &[AccountMeta], #method_args) -> Instruction {
                                // Create the instruction enum.
                                let instruction = #instruction_enum_ident::#variant_name {
                                    #method_arg_idents,
                                };
                                // Serialize.
                                let data = #coder_struct::to_bytes(instruction);
                                Instruction {
                                    program_id: program_id,
                                    data,
                                    accounts: accounts.to_vec(),
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
                                    .map_err(|e| ClientError::RpcError(e))?;
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
                                    .map_err(|e| ClientError::RpcError(e))
                            }
                        };

                        // Save the single dispatch arm representing this enum variant.
                        dispatch_arms.push(quote! {
                            #instruction_enum_ident::#variant_name {
                                #method_args
                            } => #method_name(accounts, #method_args)
                        });

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

                Some((client_methods, instruction_methods))
            }
            _ => None,
        })
        .collect::<Vec<(proc_macro2::TokenStream, proc_macro2::TokenStream)>>()
        .first()
        .expect("enum parsing failed")
        .clone();

    let decode_and_dispatch_tree = quote! {
        // Decode.
        let instruction: #instruction_enum_ident = Coder::from_bytes(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData);
        // Dispatch.
        match instruction {
            #(#dispatch_arms),*
        }
    };

    // Name of the module the macro is over.
    let instruction_mod_ident = instruction_mod.ident.clone();

    // Define the instruction coder to use for serialization.
    let coder_definition = match coder_struct_opt {
        // It's defined externally so do nothing.
        Some(_) => quote! {},
        // Coder not provided, so use declare and use the default one.
        None => quote! {
            struct _DefaultCoder;
            impl _DefaultCoder {
                pub fn to_bytes(i: #instruction_mod_ident::#instruction_enum_ident) -> Vec<u8> {
                    bincode::serialize(&(0u8, i))
                        .expect("instruction must be serializable")
                }
                pub fn from_bytes(data: &[u8]) -> Result<Vec<u8>, ()> {
                    match data.split_first() {
                        None => Err(()),
                        Some((&u08, rest)) => bincode::deserialize(rest).map_err(|_| ()),
                        Some((_, _rest)) => Err(()),
                    }
                }
            }
        },
    };

    (
        client_methods,
        instruction_methods,
        decode_and_dispatch_tree,
        coder_definition,
    )
}
