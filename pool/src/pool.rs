use arrayref::array_refs;

use serum_pool_schema::{
	PoolRequest,
	PoolRequestInner,
};

use solana_sdk::{
	entrypoint::ProgramResult,
	account_info::AccountInfo,
	pubkey::Pubkey,
};

use std::error::Error;

pub trait Pool {
	fn process_pool_request(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
		request: &PoolRequest,
	) -> ProgramResult;

	fn process_other_instruction(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
		instruction_data: &[u8],
	) -> ProgramResult;
}

pub trait PoolEasy {
	fn process_other_instruction(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
		instruction_data: &[u8],
	) -> ProgramResult;

	// TODO add stuff to this trait as needed for the impl below
}

impl<P: PoolEasy> Pool for P {
	fn process_other_instruction(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
		instruction_data: &[u8],
	) -> ProgramResult {
		<P as PoolEasy>::process_other_instruction(program_id, accounts, instruction_data)
	}

	fn process_pool_request(
		program_id: &Pubkey,
		accounts: &[AccountInfo],
		request: &PoolRequest,
	) -> ProgramResult {
		// TODO custom error type
		let (pool_account, accounts) = accounts.split_at(1);
		let state = unimplemented!();
		match request.inner {
			PoolRequestInner::GetInitializeParams => unimplemented!(),
			PoolRequestInner::Initialize => unimplemented!(),
			PoolRequestInner::GetBasket(_) => unimplemented!(),
			PoolRequestInner::Transact(_) => unimplemented!(),
			PoolRequestInner::AdminRequest => unimplemented!(),
		}
	}
}
