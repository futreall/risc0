// Copyright 2024 RISC Zero, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#[cfg(test)]
#[cfg(feature = "prove")]
mod tests;

#[cfg(test)]
#[cfg(feature = "prove")]
#[cfg(feature = "bigint-dig-shim")]
mod tests_dig;

#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
use anyhow::{bail, Result};
use num_bigint::BigUint;
#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
#[cfg(feature = "bigint-dig-shim")]
use num_bigint_dig::BigUint as BigUintDig;
#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
use risc0_zkvm_platform::{
    syscall::{rsa::WIDTH_BITS, rsa::WIDTH_WORDS, sys_rsa},
    WORD_SIZE,
};

#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
use crate::prove;
use crate::{BigIntClaim, BigIntProgram};

// Re-export program info
pub use crate::generated::{RSA_256_X1, RSA_256_X2, RSA_3072_X1, RSA_3072_X15};

/// Construct a bigint claim that (S^e = M (mod N)), where e = 65537.
///
/// `S` is the `base``, `N` is the `modulus`, and `M` is the `result`
pub fn claim(
    prog_info: &BigIntProgram,
    modulus: BigUint,
    base: BigUint,
    result: BigUint,
) -> BigIntClaim {
    BigIntClaim::from_biguints(prog_info, &[modulus, base, result])
}

/// Compute M = S^e (mod N), where e = 65537, including an accelerated proof that the computation is correct
///
/// `S` is the `base`, `N` is the `modulus`, and the result `M` is returned
#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
#[cfg(not(feature = "bigint-dig-shim"))]
pub fn modpow_65537(base: &BigUint, modulus: &BigUint) -> Result<BigUint> {
    let claims = compute_claim_inner(base.to_u32_digits(), modulus.to_u32_digits())?;
    let result = claims[2].clone();
    let claims = BigIntClaim::from_biguints(&RSA_3072_X1, &claims);
    prove(&RSA_3072_X1, &[claims]).expect("Unable to compose with RSA");
    return Ok(result);
}

/// Compute M = S^e (mod N), where e = 65537, including an accelerated proof that the computation is correct
///
/// `S` is the `base`, `N` is the `modulus`, and the result `M` is returned
#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
#[cfg(feature = "bigint-dig-shim")]
pub fn modpow_65537(base: &BigUintDig, modulus: &BigUintDig) -> Result<BigUintDig> {
    // Ensure inputs fill an even number of words
    let mut base = base.to_bytes_le();
    if base.len() % 4 != 0 {
        base.resize(base.len() + (4 - (base.len() % 4)), 0);
    }
    let mut modulus = modulus.to_bytes_le();
    if modulus.len() % 4 != 0 {
        modulus.resize(modulus.len() + (4 - (modulus.len() % 4)), 0);
    }
    // Convert inputs to Vecs of u32s
    let mut base_vec = Vec::new();
    for word in base.chunks(4) {
        let word: [u8; 4] = word.try_into()?;
        base_vec.push(u32::from_le_bytes(word));
    }
    let mut modulus_vec = Vec::<u32>::new();
    for word in modulus.chunks(4) {
        let word: [u8; 4] = word.try_into()?;
        modulus_vec.push(u32::from_le_bytes(word));
    }
    let claims = compute_claim_inner(base_vec, modulus_vec)?;
    let result = BigUintDig::from_bytes_le(&claims[2].to_bytes_le()).clone();
    let claims = BigIntClaim::from_biguints(&RSA_3072_X1, &claims);
    prove(&RSA_3072_X1, &[claims]).expect("Unable to compose with RSA");
    return Ok(result);
}

/// Compute M = S^e (mod N), where e = 65537, and return the `claim` to prove this
///
/// `S` is the `base` and `N` is the `modulus`.
///
/// The return value has the claim inputs expected by the RSA accelerator, in the expected order, which is [modulus, base, result]
#[cfg(all(target_os = "zkvm", target_arch = "riscv32"))]
fn compute_claim_inner(mut base: Vec<u32>, mut modulus: Vec<u32>) -> Result<[BigUint; 3]> {
    assert!(WORD_SIZE == 4);
    if modulus.len() > WIDTH_WORDS || base.len() > WIDTH_WORDS {
        bail!("RSA acceleration supports up to {} bits, but received {} u32s for the modulus and {} u32s for the base.", WIDTH_BITS, modulus.len(), base.len());
    }
    modulus.resize(WIDTH_WORDS, 0);
    base.resize(WIDTH_WORDS, 0);
    let modulus: [u32; WIDTH_WORDS] = (*modulus).try_into()?;
    let base: [u32; WIDTH_WORDS] = (*base).try_into()?;
    const fn zero() -> u32 {
        0
    }
    let mut result = [zero(); WIDTH_WORDS];
    // Safety: inputs are aligned and dereferenceable
    unsafe {
        sys_rsa(&mut result, &base, &modulus);
    }
    let result = result
        .iter()
        .flat_map(|elem| elem.to_le_bytes())
        .collect::<Vec<u8>>();
    let result = BigUint::from_bytes_le(&result);
    let base = base
        .iter()
        .flat_map(|elem| elem.to_le_bytes())
        .collect::<Vec<u8>>();
    let base = BigUint::from_bytes_le(&base);
    let modulus = modulus
        .iter()
        .flat_map(|elem| elem.to_le_bytes())
        .collect::<Vec<u8>>();
    let modulus = BigUint::from_bytes_le(&modulus);
    Ok([modulus, base, result])
}
