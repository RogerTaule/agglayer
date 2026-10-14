//! ZisK facade: program execution and program verification keys. Proof
//! generation and verification are not part of the node yet.
//!
//! All crates outside of the pessimistic-proof family should reach ZisK
//! through this crate and avoid direct `zisk-*` dependencies.

use zisk_core::ZiskRom;
use zisk_sdk::GuestProgram;
pub use zisk_sdk::{HashMode, ProgramVK, ZiskStdin};
use zisk_transpiler_riscv::Riscv2zisk;
use ziskemu::{EmuOptions, ZiskEmulator};

/// ZisK version the host crates and the guest program are built with. Keep in
/// sync with the `zisk-*` versions pinned in the workspace.
pub const ZISK_VERSION: &str = "1.3.2-alpha";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to transpile the ELF into a ZisK ROM: {0}")]
    Transpile(String),
    #[error("ZisK emulation failed: {0}")]
    Emulation(String),
    #[error("failed to compute the program verification key: {0}")]
    ProgramVk(String),
}

/// Runs a program in the ZisK emulator.
///
/// This needs no prover client.
pub struct Executor {
    rom: ZiskRom,
}

impl Executor {
    /// Transpiles `elf` into a ZisK ROM, once for all executions.
    pub fn new(elf: &[u8]) -> Result<Self, Error> {
        let rom = Riscv2zisk::new(elf)
            .run()
            .map_err(|error| Error::Transpile(error.to_string()))?;
        Ok(Self { rom })
    }

    /// Runs the program with `stdin` and returns its public outputs.
    ///
    /// A guest that panics, for example on an invalid certificate, makes the
    /// emulator panic on the host instead of returning an error, so the panic
    /// is caught and reported as [`Error::Emulation`].
    pub fn execute(&self, stdin: &ZiskStdin) -> Result<Vec<u8>, Error> {
        let inputs = stdin.read_data();

        std::panic::catch_unwind(|| {
            ZiskEmulator::process_rom(&self.rom, &inputs, &EmuOptions::default(), None::<fn(_)>)
        })
        .map_err(|_| Error::Emulation("the guest program panicked".to_string()))?
        .map_err(|error| Error::Emulation(format!("{error:?}")))
    }
}

/// Computes the program verification key of `elf` under `hash_mode`.
pub fn program_vk(elf: &[u8], hash_mode: HashMode) -> Result<ProgramVK, Error> {
    GuestProgram::from_bytes("program", elf.to_vec())
        .vk_with_mode(hash_mode)
        .map_err(|error| Error::ProgramVk(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pinned_version(manifest: &str, path: &[&str]) -> String {
        let manifest: toml::Value = toml::from_str(manifest).unwrap();
        let value = path.iter().fold(&manifest, |value, key| &value[key]);
        value.as_str().unwrap().trim_start_matches('=').to_string()
    }

    /// `ZISK_VERSION` is recorded in every stored proof: it must match the
    /// ZisK crates the node is built with.
    #[test]
    fn zisk_version_matches_the_pinned_crates() {
        let workspace = include_str!("../../../../Cargo.toml");

        assert_eq!(
            pinned_version(workspace, &["workspace", "dependencies", "zisk-sdk"]),
            ZISK_VERSION
        );
    }
}
