use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{ensure, Result};
use filecoin_proofs_v1::constants::{
    SectorShape2KiB, SectorShape32GiB, SectorShape512MiB, SectorShape8MiB,
};
use filecoin_proofs_v1::storage_proofs::hasher::Hasher;
use filecoin_proofs_v1::types::MerkleTreeTrait;
use filecoin_proofs_v1::types::VanillaSealProof as RawVanillaSealProof;
use filecoin_proofs_v1::{with_shape, Labels as RawLabels};
use serde::{Deserialize, Serialize};

use crate::{
    Commitment, PieceInfo, ProverId, RegisteredSealProof, SectorId, Ticket, UnpaddedByteIndex,
    UnpaddedBytesAmount, Version,
};

/// The output of `seal_pre_commit_phase1`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealPreCommitPhase1Output {
    pub registered_proof: RegisteredSealProof,
    pub labels: Labels,
    pub config: filecoin_proofs_v1::StoreConfig,
    pub comm_d: filecoin_proofs_v1::Commitment,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Labels {
    StackedDrg2KiBV1(RawLabels<SectorShape2KiB>),
    StackedDrg8MiBV1(RawLabels<SectorShape8MiB>),
    StackedDrg512MiBV1(RawLabels<SectorShape512MiB>),
    StackedDrg32GiBV1(RawLabels<SectorShape32GiB>),
}

fn convert_labels<Tree: 'static + MerkleTreeTrait>(
    proof: RegisteredSealProof,
    labels: &RawLabels<Tree>,
) -> Labels {
    use std::any::Any;
    use RegisteredSealProof::*;
    match proof {
        StackedDrg2KiBV1 => {
            if let Some(labels) = Any::downcast_ref::<RawLabels<SectorShape2KiB>>(labels) {
                Labels::StackedDrg2KiBV1(labels.clone())
            } else {
                panic!("invalid labels provided")
            }
        }
        StackedDrg8MiBV1 => {
            if let Some(labels) = Any::downcast_ref::<RawLabels<SectorShape8MiB>>(labels) {
                Labels::StackedDrg8MiBV1(labels.clone())
            } else {
                panic!("invalid labels provided")
            }
        }
        StackedDrg512MiBV1 => {
            if let Some(labels) = Any::downcast_ref::<RawLabels<SectorShape512MiB>>(labels) {
                Labels::StackedDrg512MiBV1(labels.clone())
            } else {
                panic!("invalid labels provided")
            }
        }
        StackedDrg32GiBV1 => {
            if let Some(labels) = Any::downcast_ref::<RawLabels<SectorShape32GiB>>(labels) {
                Labels::StackedDrg32GiBV1(labels.clone())
            } else {
                panic!("invalid labels provided")
            }
        }
    }
}

// TODO: avoid panic and use try_into
impl<Tree: 'static + MerkleTreeTrait> Into<RawLabels<Tree>> for Labels {
    fn into(self) -> RawLabels<Tree> {
        use std::any::Any;
        use Labels::*;

        match self {
            StackedDrg2KiBV1(raw) => {
                if let Some(raw) = Any::downcast_ref::<RawLabels<Tree>>(&raw) {
                    raw.clone()
                } else {
                    panic!("cannot convert 2kib into different structure")
                }
            }
            StackedDrg8MiBV1(raw) => {
                if let Some(raw) = Any::downcast_ref::<RawLabels<Tree>>(&raw) {
                    raw.clone()
                } else {
                    panic!("cannot convert 8Mib into different structure")
                }
            }
            StackedDrg512MiBV1(raw) => {
                if let Some(raw) = Any::downcast_ref::<RawLabels<Tree>>(&raw) {
                    raw.clone()
                } else {
                    panic!("cannot convert 512Mib into different structure")
                }
            }
            StackedDrg32GiBV1(raw) => {
                if let Some(raw) = Any::downcast_ref::<RawLabels<Tree>>(&raw) {
                    raw.clone()
                } else {
                    panic!("cannot convert 32gib into different structure")
                }
            }
        }
    }
}

/// The output of `seal_pre_commit_phase2`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealPreCommitPhase2Output {
    pub registered_proof: RegisteredSealProof,
    pub comm_r: Commitment,
    pub comm_d: Commitment,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealCommitPhase1Output {
    pub registered_proof: RegisteredSealProof,
    pub vanilla_proofs: Vec<Vec<VanillaSealProof>>,
    pub comm_r: Commitment,
    pub comm_d: Commitment,
    pub replica_id: <filecoin_proofs_v1::constants::DefaultTreeHasher as Hasher>::Domain,
    pub seed: Ticket,
    pub ticket: Ticket,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum VanillaSealProof {
    StackedDrg2KiBV1(RawVanillaSealProof<SectorShape2KiB>),
    StackedDrg8MiBV1(RawVanillaSealProof<SectorShape8MiB>),
    StackedDrg512MiBV1(RawVanillaSealProof<SectorShape512MiB>),
    StackedDrg32GiBV1(RawVanillaSealProof<SectorShape32GiB>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SealCommitPhase2Output {
    pub proof: Vec<u8>,
}

pub fn clear_cache(sector_size: u64, cache_path: &Path) -> Result<()> {
    use filecoin_proofs_v1::{clear_cache, with_shape};
    with_shape!(sector_size, clear_cache, cache_path)
}

pub fn seal_pre_commit_phase1<R, S, T>(
    registered_proof: RegisteredSealProof,
    cache_path: R,
    in_path: S,
    out_path: T,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: Ticket,
    piece_infos: &[PieceInfo],
) -> Result<SealPreCommitPhase1Output>
where
    R: AsRef<Path>,
    S: AsRef<Path>,
    T: AsRef<Path>,
{
    ensure!(
        registered_proof.version() == Version::V1,
        "unusupported version"
    );

    with_shape!(
        u64::from(registered_proof.sector_size()),
        seal_pre_commit_phase1_inner,
        registered_proof,
        cache_path.as_ref(),
        in_path.as_ref(),
        out_path.as_ref(),
        prover_id,
        sector_id,
        ticket,
        piece_infos
    )
}

fn seal_pre_commit_phase1_inner<Tree: 'static + MerkleTreeTrait>(
    registered_proof: RegisteredSealProof,
    cache_path: &Path,
    in_path: &Path,
    out_path: &Path,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: Ticket,
    piece_infos: &[PieceInfo],
) -> Result<SealPreCommitPhase1Output> {
    let config = registered_proof.as_v1_config();

    let output = filecoin_proofs_v1::seal_pre_commit_phase1::<_, _, _, Tree>(
        config,
        cache_path,
        in_path,
        out_path,
        prover_id,
        sector_id,
        ticket,
        piece_infos,
    )?;

    let filecoin_proofs_v1::types::SealPreCommitPhase1Output::<Tree> {
        labels,
        config,
        comm_d,
    } = output;

    Ok(SealPreCommitPhase1Output {
        registered_proof,
        labels: convert_labels::<Tree>(registered_proof, &labels),
        config,
        comm_d,
    })
}

pub fn seal_pre_commit_phase2<R, S>(
    phase1_output: SealPreCommitPhase1Output,
    cache_path: R,
    out_path: S,
) -> Result<SealPreCommitPhase2Output>
where
    R: AsRef<Path>,
    S: AsRef<Path>,
{
    ensure!(
        phase1_output.registered_proof.version() == Version::V1,
        "unusupported version"
    );

    with_shape!(
        u64::from(phase1_output.registered_proof.sector_size()),
        seal_pre_commit_phase2_inner,
        phase1_output,
        cache_path.as_ref(),
        out_path.as_ref(),
    )
}

fn seal_pre_commit_phase2_inner<Tree: 'static + MerkleTreeTrait>(
    phase1_output: SealPreCommitPhase1Output,
    cache_path: &Path,
    out_path: &Path,
) -> Result<SealPreCommitPhase2Output> {
    use RegisteredSealProof::*;
    let SealPreCommitPhase1Output {
        registered_proof,
        labels,
        config,
        comm_d,
    } = phase1_output;

    let seal_pre_commit_phase1_output =
        filecoin_proofs_v1::types::SealPreCommitPhase1Output::<Tree> {
            labels: labels.into(),
            config,
            comm_d,
        };

    filecoin_proofs_v1::validate_cache_for_precommit_phase2::<_, _, Tree>(
        &cache_path,
        &out_path,
        &seal_pre_commit_phase1_output,
    )?;

    let output = filecoin_proofs_v1::seal_pre_commit_phase2::<_, _, Tree>(
        registered_proof.as_v1_config(),
        seal_pre_commit_phase1_output,
        cache_path,
        out_path,
    )?;

    let filecoin_proofs_v1::types::SealPreCommitOutput { comm_d, comm_r } = output;

    Ok(SealPreCommitPhase2Output {
        registered_proof,
        comm_d,
        comm_r,
    })
}

pub fn compute_comm_d(
    registered_proof: RegisteredSealProof,
    piece_infos: &[PieceInfo],
) -> Result<Commitment> {
    filecoin_proofs_v1::compute_comm_d(registered_proof.sector_size(), piece_infos)
}

pub fn seal_commit_phase1<T: AsRef<Path>>(
    cache_path: T,
    replica_path: T,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: Ticket,
    seed: Ticket,
    pre_commit: SealPreCommitPhase2Output,
    piece_infos: &[PieceInfo],
) -> Result<SealCommitPhase1Output> {
    todo!()
    // let SealPreCommitPhase2Output {
    //     comm_r,
    //     comm_d,
    //     registered_proof,
    // } = pre_commit;
    // use RegisteredSealProof::*;
    // match registered_proof {
    //     StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
    //         let config = registered_proof.as_v1_config();
    //         let pc = filecoin_proofs_v1::types::SealPreCommitOutput { comm_r, comm_d };

    //         filecoin_proofs_v1::validate_cache_for_commit(&cache_path, &replica_path)?;

    //         let output = filecoin_proofs_v1::seal_commit_phase1(
    //             config,
    //             cache_path,
    //             replica_path,
    //             prover_id,
    //             sector_id,
    //             ticket,
    //             seed,
    //             pc,
    //             piece_infos,
    //         )?;

    //         let filecoin_proofs_v1::types::SealCommitPhase1Output {
    //             vanilla_proofs,
    //             comm_r,
    //             comm_d,
    //             replica_id,
    //             seed,
    //             ticket,
    //         } = output;

    //         Ok(SealCommitPhase1Output {
    //             registered_proof,
    //             vanilla_proofs,
    //             comm_r,
    //             comm_d,
    //             replica_id,
    //             seed,
    //             ticket,
    //         })
    //     }
    // }
}

pub fn seal_commit_phase2(
    phase1_output: SealCommitPhase1Output,
    prover_id: ProverId,
    sector_id: SectorId,
) -> Result<SealCommitPhase2Output> {
    todo!()
    // let SealCommitPhase1Output {
    //     vanilla_proofs,
    //     comm_r,
    //     comm_d,
    //     replica_id,
    //     seed,
    //     ticket,
    //     registered_proof,
    // } = phase1_output;
    // use RegisteredSealProof::*;
    // match registered_proof {
    //     StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
    //         let config = registered_proof.as_v1_config();
    //         let co = filecoin_proofs_v1::types::SealCommitPhase1Output {
    //             vanilla_proofs,
    //             comm_r,
    //             comm_d,
    //             replica_id,
    //             seed,
    //             ticket,
    //         };

    //         let output = filecoin_proofs_v1::seal_commit_phase2(config, co, prover_id, sector_id)?;

    //         Ok(SealCommitPhase2Output {
    //             proof: output.proof,
    //         })
    //     }
    // }
}

pub fn verify_seal(
    registered_proof: RegisteredSealProof,
    comm_r_in: Commitment,
    comm_d_in: Commitment,
    prover_id: ProverId,
    sector_id: SectorId,
    ticket: Ticket,
    seed: Ticket,
    proof_vec: &[u8],
) -> Result<bool> {
    todo!()
    // use RegisteredSealProof::*;
    // match registered_proof {
    //     StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
    //         let config = registered_proof.as_v1_config();

    //         filecoin_proofs_v1::verify_seal(
    //             config, comm_r_in, comm_d_in, prover_id, sector_id, ticket, seed, proof_vec,
    //         )
    //     }
    // }
}

pub fn verify_batch_seal(
    registered_proof: RegisteredSealProof,
    comm_r_ins: &[Commitment],
    comm_d_ins: &[Commitment],
    prover_ids: &[ProverId],
    sector_ids: &[SectorId],
    tickets: &[Ticket],
    seeds: &[Ticket],
    proof_vecs: &[&[u8]],
) -> Result<bool> {
    todo!()
    // use RegisteredSealProof::*;
    // match registered_proof {
    //     StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
    //         let config = registered_proof.as_v1_config();

    //         filecoin_proofs_v1::verify_batch_seal(
    //             config, comm_r_ins, comm_d_ins, prover_ids, sector_ids, tickets, seeds, proof_vecs,
    //         )
    //     }
    // }
}

pub fn get_unsealed_range<T: Into<PathBuf> + AsRef<Path>>(
    registered_proof: RegisteredSealProof,
    cache_path: T,
    sealed_path: T,
    output_path: T,
    prover_id: ProverId,
    sector_id: SectorId,
    comm_d: Commitment,
    ticket: Ticket,
    offset: UnpaddedByteIndex,
    num_bytes: UnpaddedBytesAmount,
) -> Result<UnpaddedBytesAmount> {
    todo!()
    // use RegisteredSealProof::*;
    // match registered_proof {
    //     StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
    //         let config = registered_proof.as_v1_config();

    //         filecoin_proofs_v1::get_unsealed_range(
    //             config,
    //             cache_path,
    //             sealed_path,
    //             output_path,
    //             prover_id,
    //             sector_id,
    //             comm_d,
    //             ticket,
    //             offset,
    //             num_bytes,
    //         )
    //     }
    // }
}

pub fn generate_piece_commitment<T: Read>(
    registered_proof: RegisteredSealProof,
    source: T,
    piece_size: UnpaddedBytesAmount,
) -> Result<PieceInfo> {
    use RegisteredSealProof::*;
    match registered_proof {
        StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
            filecoin_proofs_v1::generate_piece_commitment(source, piece_size)
        }
    }
}

pub fn add_piece<R, W>(
    registered_proof: RegisteredSealProof,
    source: R,
    target: W,
    piece_size: UnpaddedBytesAmount,
    piece_lengths: &[UnpaddedBytesAmount],
) -> Result<(PieceInfo, UnpaddedBytesAmount)>
where
    R: Read,
    W: Read + Write + Seek,
{
    use RegisteredSealProof::*;
    match registered_proof {
        StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
            filecoin_proofs_v1::add_piece(source, target, piece_size, piece_lengths)
        }
    }
}

pub fn write_and_preprocess<R, W>(
    registered_proof: RegisteredSealProof,
    source: R,
    target: W,
    piece_size: UnpaddedBytesAmount,
) -> Result<(PieceInfo, UnpaddedBytesAmount)>
where
    R: Read,
    W: Read + Write + Seek,
{
    use RegisteredSealProof::*;
    match registered_proof {
        StackedDrg2KiBV1 | StackedDrg8MiBV1 | StackedDrg512MiBV1 | StackedDrg32GiBV1 => {
            filecoin_proofs_v1::write_and_preprocess(source, target, piece_size)
        }
    }
}
