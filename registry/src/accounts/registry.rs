use arrayref::{array_mut_ref, array_ref, array_refs, mut_array_refs};
use serum_common::pack::*;
use solana_client_gen::solana_sdk::pubkey::Pubkey;
use std::io::Read;

pub const SIZE: usize = 482;

// TODO: BPF overflows if we jack this up too high. Need to use raw bytes
//       and manually index/deserialize on demand if we want to increase the
//       length and keep the data structure as [Pubkey; CAPABILITIES_LEN].
pub const CAPABILITIES_LEN: usize = 10;

/// Index of the nonce byte when the Registry is serialized in account data.
pub const NONCE_INDEX: usize = 65;

/// Registry defines the account representing an instance of the program.
#[derive(Clone, Debug, PartialEq)]
pub struct Registry {
    /// Set by the program on initialization.
    pub initialized: bool,
    /// The mint of the SPL token used by the registry (SRM).
    pub mint: Pubkey,
    /// The mint of the mega SPL token used by the registry (MSRM).
    pub mega_mint: Pubkey,
    /// The nonce for the program-derived-address controlling the token
    /// vault.
    pub nonce: u8,
    /// Maps capability identifier to the Pubkey address of the program
    /// to calculate rewards for the capability.
    pub capabilities: [Pubkey; CAPABILITIES_LEN],
    /// The priviledged account with the ability to register capabilities.
    pub authority: Pubkey,
    /// Rewards program id.
    pub rewards: Pubkey,
    /// Rewards ReturnValue account.
    pub rewards_return_value: Pubkey,
}

impl Pack for Registry {
    fn pack(src: Registry, dst: &mut [u8]) -> Result<(), ProgramError> {
        if src.size()? != dst.len() as u64 {
            return Err(ProgramError::InvalidAccountData);
        }

        let dst = array_mut_ref![dst, 0, SIZE];
        let (
            initialized_dst,
            mint_dst,
            mega_mint_dst,
            nonce_dst,
            capabilities_dst,
            authority_dst,
            rewards_dst,
            rewards_return_value_dst,
        ) = mut_array_refs![dst, 1, 32, 32, 1, 320, 32, 32, 32];

        let Registry {
            initialized,
            mint,
            mega_mint,
            nonce,
            capabilities,
            authority,
            rewards,
            rewards_return_value,
        } = src;

        initialized_dst[0] = initialized as u8;
        mint_dst.copy_from_slice(mint.as_ref());
        mega_mint_dst.copy_from_slice(mega_mint.as_ref());
        nonce_dst[0] = nonce as u8;

        for (idx, c) in capabilities.iter().enumerate() {
            let pos = idx * 32;
            capabilities_dst[pos..pos + 32].copy_from_slice(c.as_ref());
        }

        authority_dst.copy_from_slice(authority.as_ref());
        rewards_dst.copy_from_slice(rewards.as_ref());
        rewards_return_value_dst.copy_from_slice(rewards_return_value.as_ref());

        Ok(())
    }

    fn unpack_unchecked(src: &mut &[u8]) -> Result<Registry, ProgramError> {
        // TODO: don't read into this intermedite array.
        let mut new_src = vec![];
        src.read_to_end(&mut new_src)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let new_src = array_ref![new_src, 0, SIZE];
        let (
            initialized,
            mint,
            mega_mint,
            nonce,
            capabilities_bytes,
            authority,
            rewards,
            rewards_return_value,
        ) = array_refs![new_src, 1, 32, 32, 1, 320, 32, 32, 32];

        let mut capabilities = [Pubkey::new_from_array([0; 32]); CAPABILITIES_LEN];
        let mut idx = 0;
        for byte_idx in (0..capabilities_bytes.len()).step_by(32) {
            let end = byte_idx + 32;
            let c = Pubkey::new(&capabilities_bytes[byte_idx..end]);
            capabilities[idx] = c;
            idx += 1;
        }

        let r = Registry {
            initialized: match initialized {
                [0] => false,
                [1] => true,
                _ => return Err(ProgramError::InvalidAccountData),
            },
            mint: Pubkey::new(mint),
            mega_mint: Pubkey::new(mega_mint),
            nonce: nonce[0],
            capabilities,
            authority: Pubkey::new(authority),
            rewards: Pubkey::new(rewards),
            rewards_return_value: Pubkey::new(rewards_return_value),
        };

        Ok(r)
    }

    fn size(&self) -> Result<u64, ProgramError> {
        Ok(SIZE as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_unpack() {
        let reg = Registry {
            initialized: true,
            mint: Pubkey::new_rand(),
            mega_mint: Pubkey::new_rand(),
            nonce: 1,
            capabilities: [Pubkey::new_rand(); CAPABILITIES_LEN],
            authority: Pubkey::new_rand(),
            rewards: Pubkey::new_rand(),
            rewards_return_value: Pubkey::new_rand(),
        };
        let mut data = [0u8; SIZE];
        Registry::pack(reg.clone(), &mut data).unwrap();
        let new_reg = Registry::unpack(&data).unwrap();

        assert_eq!(reg, new_reg);
    }
}
