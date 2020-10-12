use crate::error::{LockupError, LockupErrorCode};
use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use borsh::{BorshDeserialize, BorshSchema, BorshSerialize};
use serum_common::pack::*;
use solana_client_gen::prelude::*;
use solana_client_gen::solana_sdk::account_info::AccountInfo;

/// Whitelist maintaining the list of program-derived-addresses the Locked
/// SRM program is allowed to delegate funds to. This is used, for example,
/// to allow locked SRM to be sent to the staking program.
///
/// Note that the whitelist backing storage is too large to be able to pack/unpack
/// it on the BPF stack. As a result, we just wrap the raw data array
/// and access the data as needed with the api accessors provided here.
///
/// This makes it a bit unsafe to use--since Solana's data storage
/// is wrapped in a RefCell, so be careful when you're mutating the
/// whitelist to avoid a RefCell induced panic.
#[derive(Debug)]
pub struct Whitelist<'a> {
    // Account layout: SAFE_PUBKEY || ..WHITELIST_ENTRY
    pub acc_info: AccountInfo<'a>,
}

impl<'a> Whitelist<'a> {
    /// Index at which the whitelist entries start.
    pub const ITEM_START: usize = 32;
    /// Byte size for a single item in the whitelist.
    pub const ITEM_SIZE: usize = 65;
    /// Number of items in the whitelist.
    pub const LEN: usize = 50; // TODO: how big do we want this?
    /// Byte size of the entire whitelist account.
    pub const SIZE: usize = Whitelist::ITEM_START + 65 * Whitelist::LEN;

    pub fn new(acc_info: AccountInfo<'a>) -> Result<Self, LockupError> {
        if acc_info.try_data_len()? != Whitelist::SIZE {
            return Err(LockupErrorCode::WhitelistInvalidData)?;
        }
        Ok(Self { acc_info })
    }

    /// Returns the address of the Safe account this Whitelist belongs to.
    pub fn safe(&self) -> Result<Pubkey, LockupError> {
        let data = self.acc_info.try_borrow_data()?;
        Ok(Pubkey::new(&data[..Self::ITEM_START]))
    }

    /// Sets the safe address on this whitelist .
    pub fn set_safe(&self, safe: &Pubkey) -> Result<(), LockupError> {
        let mut data = self.acc_info.try_borrow_mut_data()?;
        let dst = array_mut_ref![data, 0, Whitelist::ITEM_START];
        dst.copy_from_slice(safe.as_ref());
        Ok(())
    }

    /// Returns the WhitelistEntry at the given index.
    pub fn get_at(&self, index: usize) -> Result<WhitelistEntry, LockupError> {
        let data = &self.acc_info.try_borrow_data()?;
        let start = Whitelist::ITEM_START + index * Whitelist::ITEM_SIZE;
        let new_slice = array_ref![data, start, Whitelist::ITEM_SIZE];
        let (program_id, instance, nonce) = array_refs![&new_slice, 32, 32, 1];
        Ok(WhitelistEntry::new(
            Pubkey::new(program_id),
            Pubkey::new(instance),
            nonce[0],
        ))
    }

    /// Inserts the given WhitelistEntry at the first available index.
    /// Returns Some(index) where the entry was inserted. If the Whitelist
    /// is full, returns None.
    pub fn push(&self, entry: WhitelistEntry) -> Result<Option<usize>, LockupError> {
        let existing_idx = self.index_of(&entry)?;
        if let Some(_) = existing_idx {
            return Err(LockupErrorCode::WhitelistEntryAlreadyExists)?;
        }
        let idx = self.index_of(&WhitelistEntry::zero())?;
        if let Some(idx) = idx {
            self.add_at(idx, entry)?;
            return Ok(Some(idx));
        }
        Ok(idx)
    }

    /// Deletes the given entry from the Whitelist.
    pub fn delete(&self, entry: WhitelistEntry) -> Result<Option<usize>, LockupError> {
        let idx = self.index_of(&entry)?;
        if let Some(idx) = idx {
            self.add_at(idx, WhitelistEntry::zero())?;
            return Ok(Some(idx));
        }
        Ok(idx)
    }

    /// Returns the entry representing the given derived address. If no such
    /// entry exists, returns Ok(None).
    pub fn get_derived(&self, derived: &Pubkey) -> Result<Option<WhitelistEntry>, LockupError> {
        for k in (Whitelist::ITEM_START..Whitelist::SIZE).step_by(Whitelist::ITEM_SIZE) {
            let curr_idx = (k - Whitelist::ITEM_START) / Whitelist::ITEM_SIZE;
            let entry = self.get_at(curr_idx)?;
            if entry != WhitelistEntry::zero() && &entry.derived_address()? == derived {
                return Ok(Some(entry));
            }
        }
        Ok(None)
    }

    /// Returns true if a WhitelistEntry representing the given derived address
    /// exists in the Whitelist.
    pub fn contains_derived(&self, derived: &Pubkey) -> Result<bool, LockupError> {
        self.get_derived(derived).map(|o| o.is_some())
    }

    // Inserts the given WhitelistEntry at the given index.
    fn add_at(&self, index: usize, item: WhitelistEntry) -> Result<(), LockupError> {
        let data = &mut self.acc_info.try_borrow_mut_data()?;
        let start = Whitelist::ITEM_START + index * Whitelist::ITEM_SIZE;
        let dst = array_mut_ref![data, start, Whitelist::ITEM_SIZE];
        let (program_id_dst, instance_dst, nonce) = mut_array_refs![dst, 32, 32, 1];
        program_id_dst.copy_from_slice(item.program_id().as_ref());
        instance_dst.copy_from_slice(item.instance().as_ref());
        nonce[0] = item.nonce();
        Ok(())
    }

    fn index_of(&self, e: &WhitelistEntry) -> Result<Option<usize>, LockupError> {
        for k in (Whitelist::ITEM_START..Whitelist::SIZE).step_by(Whitelist::ITEM_SIZE) {
            let curr_idx = (k - Whitelist::ITEM_START) / Whitelist::ITEM_SIZE;
            let entry = &self.get_at(curr_idx)?;
            if entry == e {
                return Ok(Some(curr_idx));
            }
        }
        Ok(None)
    }
}

/// WhitelistEntry consists of the components required to generate a program-
/// derived address: program-id and the signer seeds. The signer seeds are
/// assumed to be an additional pubkey and a nonce.
///
/// We store this rather than the derived address for inspectibility.
#[derive(Clone, Debug, PartialEq, BorshSerialize, BorshDeserialize, BorshSchema)]
pub struct WhitelistEntry {
    program_id: Pubkey,
    instance: Pubkey,
    nonce: u8,
}

impl WhitelistEntry {
    pub fn new(program_id: Pubkey, instance: Pubkey, nonce: u8) -> Self {
        Self {
            program_id,
            instance,
            nonce,
        }
    }
    pub fn program_id(&self) -> Pubkey {
        self.program_id
    }
    pub fn instance(&self) -> Pubkey {
        self.instance
    }
    pub fn nonce(&self) -> u8 {
        self.nonce
    }
    pub fn derived_address(&self) -> Result<Pubkey, LockupError> {
        Pubkey::create_program_address(
            &[self.instance().as_ref(), bytemuck::bytes_of(&self.nonce())],
            &self.program_id(),
        )
        .map_err(|_| LockupErrorCode::InvalidWhitelistEntry.into())
    }

    pub fn zero() -> Self {
        WhitelistEntry {
            program_id: Pubkey::new_from_array([0; 32]),
            instance: Pubkey::new_from_array([0; 32]),
            nonce: 0,
        }
    }
}

serum_common::packable!(WhitelistEntry);
