//! codegen defines the proc macro re-exported by solana_client_gen.
//! For documentation, see the main crate.

use heck::SnakeCase;
use proc_quote::quote;
use syn::parse::Parser;
use syn::parse_macro_input;

// At a high level, the macro works in three passes over the
// instruction enum (inside the mod).
//
// Pass 1: remove all cfg_attr from macros (but leave the macro being configured).
// Pass 2: generate code from enum variants.
// Pass 3: remove all marker attributes, e.g., #[create_account], since they are
//         not full macros by themselves.
//
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
    let mut instruction_enum_item: syn::ItemEnum = {
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

    // First pass:
    //
    // Strip out all cfg_attr's on the inner enum variants.
    // These are only used to avoid invoking a macro when compiling to bpf.
    // And can be removed once solana udpates its rust version.
    instruction_enum_item = strip_cfg_attrs(instruction_enum_item);

    // Second pass:
    //
    // Parse the instruction enum and generate code from each enum variant.
    let (client_methods, instruction_methods, decode_and_dispatch_tree) =
        enum_to_methods(&instruction_enum_item);

    // Now recreate the highest level instruction `mod`, but with our new
    // instruction_methods inside.
    let new_instruction_mod = {
        let mod_ident = instruction_mod.ident;

        // Third (and final) pass:
        //
        // Cleanse the instruction_enum of all marker attributes that are
        // used for the macro only. E.g., remove all #[create_account]
        // attributes.
        instruction_enum_item = strip_enum_markers(instruction_enum_item);

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
                            preflight_commitment: None,
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

    // Lastly, generate the api macro.
    //
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
) -> (
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
    proc_macro2::TokenStream,
) {
    // When combined together, all the dispatch arms are used on
    // program entry, to define a `match` statement to interpret an
    // instruction variant, and dispatch the request to the program's
    // corresponding api method.
    let mut dispatch_arms = vec![];

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
            // needs_account_creation is true for any enum variant
            // with a #[create_account] attribute.
            //
            // This signals that we need to generate a method that
            // inserts an *additional* instruction *before* the
            // variant's instruction to create an account.
            //
            // This would be used, for example, over the InitializeMint
            // instruction for the SPL token contract.
            let (needs_account_creation, account_data_size) = {
                let mut r = false;
                let mut account_data_size = CreateAccountDataSize::Dynamic;
                for attr in &variant.attrs {
                    if attr.path.is_ident("create_account") {
                        account_data_size = parse_create_account_attribute(attr.clone());
                        r = true;
                        break;
                    }
                }
                (r, account_data_size)
            };
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


            // Create the optional method *if* the variant contains the
            // #[create_account] attribute.
            //
            // One of two types of methods can can be created here. One
            // for accounts of fixed size and one for accounts of non-fixed
            // size.
            //
            // 1. create_account_and_<instruction-name>(...args)
            // 2. create_account_with_size_and_<instruction-name>(size: u64, ...args)
            //
            // The first will be created when #[create_account(SIZE)] is used with a
            // fixed size. The second will be used when #[create_account(..)] is used
            // with a `..`.
            let create_account_client_method = {
                let (create_account_client_method_name, create_account_client_method_name_with_signers) = {
                    match account_data_size {
                        CreateAccountDataSize::Fixed(_) => {
                            (
                                proc_macro2::Ident::new(
                                    format!("create_account_and_{}", variant_name.to_string().to_snake_case()).as_str(),
                                    proc_macro2::Span::call_site(),
                                ),
                                proc_macro2::Ident::new(
                                    format!("create_account_and_{}_with_signers", variant_name.to_string().to_snake_case()).as_str(),
                                    proc_macro2::Span::call_site(),
                                )
                            )
                        },
                        CreateAccountDataSize::Dynamic => {
                            (
                                proc_macro2::Ident::new(
                                    format!("create_account_with_size_and_{}", variant_name.to_string().to_snake_case()).as_str(),
                                    proc_macro2::Span::call_site(),
                                ),
                                // Note: the dynamic with_signers variant is not used at the moment.
                                proc_macro2::Ident::new(
                                    format!("create_account_with_size_and_{}_with_signers", variant_name.to_string().to_snake_case()).as_str(),
                                    proc_macro2::Span::call_site(),
                                )
                            )
                        },
                    }
                };
                match needs_account_creation {
                    false => quote!{},
                    true => match account_data_size {
                        CreateAccountDataSize::Fixed(account_data_size) => quote!{
                            // Inserts a create account instruction immediately before this
                            // instruction variant, so that a transaction executes twice.
                            //
                            // Note the following convention that is enforced:
                            //
                            // In the second, instruction, the new account executed will be
                            // passed into the first account slot as `writable`.
                            //
                            // The rest of the instructions for the second instruction
                            // should be passed, as usual, via the first `accounts` arg.
                            //
                            // The account created will always be rent exempt and owned by
                            // *this* program.
                            pub fn #create_account_client_method_name(
                                &self,
                                accounts: &[AccountMeta],
                                #method_args
                            ) -> Result<(Signature, Keypair), ClientError> {
                                // The new account to create.
                                let new_account = Keypair::generate(&mut OsRng);

                                let mut new_accounts = accounts.to_vec();
                                new_accounts.insert(0, AccountMeta::new(new_account.pubkey(), false));

																// This copy is most unfortunate.
                                // TODO: eliminate this clone.
                                let new_account_cpy = Keypair::from_bytes(&new_account.to_bytes()).unwrap();
                                let signers = vec![self.payer(), &new_account];
                                self.#create_account_client_method_name_with_signers(
                                    new_account_cpy,
                                    &signers,
                                    &new_accounts,
                                    #method_arg_idents
                                )
                            }
                            pub fn #create_account_client_method_name_with_signers<T: Signers>(
                                &self,
                                new_account: Keypair,
                                signers: &T,
                                accounts: &[AccountMeta],
                                #method_args
                            ) -> Result<(Signature, Keypair), ClientError> {
                                // Instruction: create the new account system instruction.
                                let create_account_instr = {
                                    let lamports = self
                                        .rpc()
                                        .get_minimum_balance_for_rent_exemption(#account_data_size as usize)
                                        .map_err(ClientError::RpcError)?;
                                    system_instruction::create_account(
                                        &self.payer().pubkey(),    // The from account on the tx.
                                        &new_account.pubkey(),     // Account to create.
                                        lamports,                  // Rent exempt balance to send to the new account.
                                        #account_data_size,        // Data init for the new acccount.
                                        self.program(),            // Owner of the new account.
                                    )
                                };

                                // Instruction: create the enum's instruction.
                                let variant_instr = super::instruction::#method_name(
                                    self.program_id,
                                    &accounts,
                                    #method_arg_idents
                                );

                                // Transaction: create the transaction with the combined instructions.
                                let tx = {
                                    let instructions = vec![create_account_instr, variant_instr];

                                    let (recent_hash, _fee_calc) = self
                                        .rpc()
                                        .get_recent_blockhash()
                                        .map_err(|e| ClientError::RawError(e.to_string()))?;
                                    Transaction::new_signed_with_payer(
                                        &instructions,
                                        Some(&self.payer().pubkey()),
                                        signers,
                                        recent_hash,
                                    )
                                };
                                // Execute the transaction.
                                self
                                    .rpc
                                    .send_and_confirm_transaction_with_spinner_and_config(
                                        &tx,
                                        self.opts.commitment,
                                        self.opts.tx,
                                    )
                                    .map_err(ClientError::RpcError)
                                    .map(|sig| (sig, new_account))
                            }

                        },
                        CreateAccountDataSize::Dynamic => quote! {
                            // Same as the fixed size version, except the first argument is size.
                            pub fn #create_account_client_method_name(&self, account_data_size: u64, accounts: &[AccountMeta], #method_args) -> Result<(Signature, Keypair), ClientError> {
                                // The new account to create.
                                let new_account = Keypair::generate(&mut OsRng);

                                // Instruction: create the new account system instruction.
                                let create_account_instr = {
                                    let lamports = self
                                        .rpc()
                                        .get_minimum_balance_for_rent_exemption(account_data_size)
                                        .map_err(ClientError::RpcError)?;
                                    system_instruction::create_account(
                                        &self.payer().pubkey(),    // The from account on the tx.
                                        &new_account.pubkey(),     // Account to create.
                                        lamports,                  // Rent exempt balance to send to the new account.
                                        account_data_size,         // Data init for the new acccount.
                                        self.program(),            // Owner of the new account.
                                    )
                                };

                                let mut new_accounts = accounts.to_vec();
                                new_accounts.insert(0, AccountMeta::new(new_account.pubkey(), false));

                                // Instruction: create the enum's instruction.
                                let variant_instr = super::instruction::#method_name(
                                    self.program_id,
                                    &new_accounts,
                                    #method_arg_idents,
                                );

                                // Transaction: create the transaction with the combined instructions.
                                let tx = {
                                    let instructions = vec![create_account_instr, variant_instr];

                                    let (recent_hash, _fee_calc) = self
                                        .rpc()
                                        .get_recent_blockhash()
                                        .map_err(|e| ClientError::RawError(e.to_string()))?;

                                    let signers = vec![self.payer(), &new_account];

                                    Transaction::new_signed_with_payer(
                                        &instructions,
                                        Some(&self.payer().pubkey()),
                                        &signers,
                                        recent_hash,
                                    )
                                };

                                // Execute the transaction.
                                self
                                    .rpc
                                    .send_and_confirm_transaction_with_spinner_and_config(
                                        &tx,
                                        self.opts.commitment,
                                        self.opts.tx,
                                    )
                                    .map_err(ClientError::RpcError)
                                    .map(|sig| (sig, new_account))
                            }
                        }
                    }
                }
            };

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

                #create_account_client_method
            };

            // Save the single dispatch arm representing this enum variant.
            dispatch_arms.push(quote! {
                #instruction_enum => #method_name(accounts, #method_args)
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

    let decode_and_dispatch_tree = quote! {
        // Decode.
        let instruction = #instruction_enum_ident::unpack(instruction_data)
            .map_err(|_| ProgramError::InvalidInstructionData);
        // Dispatch.
        match instruction {
            #(#dispatch_arms),*
        }
    };

    (
        client_methods,
        instruction_methods,
        decode_and_dispatch_tree,
    )
}

fn parse_client_ext_arg(args: proc_macro::TokenStream) -> bool {
    match args.to_string().as_ref() {
        "ext" => true,
        _ => false,
    }
}

// Parses the `SIZE`  out of the `#[create_account(SIZE)]` attribute.
//
// SIZE is used to determine the size of the account's data field,
// which is needed upon account creation.
fn parse_create_account_attribute(attr: syn::Attribute) -> CreateAccountDataSize {
    let group: proc_macro2::Group = match attr.tts.into_iter().next() {
        None => panic!("must be group deliminated"),
        Some(group) => match group {
            proc_macro2::TokenTree::Group(group) => group,
            _ => panic!("must be group delimited"),
        },
    };
    assert_eq!(group.delimiter(), proc_macro2::Delimiter::Parenthesis);
    let size_tts = group.stream();
    if size_tts.to_string() == ".." {
        return CreateAccountDataSize::Dynamic;
    }
    CreateAccountDataSize::Fixed(size_tts)
}

// Remove all attributes in the enum variants that are used for the macro only.
// Namely, the `create_account` attribute.
fn strip_enum_markers(mut instruction_enum: syn::ItemEnum) -> syn::ItemEnum {
    for variant in instruction_enum.variants.iter_mut() {
        variant.attrs = variant
            .attrs
            .iter_mut()
            .filter_map(|attr| match attr.path.is_ident("create_account") {
                true => None,
                false => Some(attr.clone()),
            })
            .collect();
    }
    instruction_enum
}

// Remove all cfg_attr attributes, leaving the attribute being configured.
// E.g., #[cfg_attr(feature = "client", create_account(10))] becomes
// #[create_account(10)].
//
// This is needed until solana updates their version of rust.
fn strip_cfg_attrs(mut instruction_enum: syn::ItemEnum) -> syn::ItemEnum {
    for variant in instruction_enum.variants.iter_mut() {
        variant.attrs = variant
            .attrs
            .iter_mut()
            .map(|attr| match attr.path.is_ident("cfg_attr") {
                true => {
                    // Assert the format is of the form:
                    // #[cfg_attr(<feature>, create_account(<input>))].
                    let mut tokens = attr.tts.to_string();
                    tokens.retain(|c| !c.is_whitespace());
                    assert!(tokens.starts_with('('));
                    assert!(tokens.ends_with(')'));
                    tokens.remove(0);
                    tokens.remove(tokens.len() - 1);
                    let parts = tokens
                        .split(',')
                        .map(|s| s.to_string())
                        .collect::<Vec<String>>();
                    assert_eq!(parts.len(), 2);
                    let create_account = &parts[1];
                    assert!(create_account.starts_with("create_account("));
                    assert!(create_account.ends_with(')'));

                    // Now create the new attribute #[create_account(<input>)].
                    let create_account_attr = format!("#[{}]", create_account);
                    let stream: proc_macro::TokenStream =
                        create_account_attr.as_str().parse().unwrap();

                    let parser = syn::Attribute::parse_outer;

                    let new_attr = &parser.parse(stream).unwrap()[0];
                    new_attr.clone()
                }
                false => attr.clone(),
            })
            .collect();
    }
    instruction_enum
}

enum CreateAccountDataSize {
    Fixed(proc_macro2::TokenStream),
    Dynamic,
}
