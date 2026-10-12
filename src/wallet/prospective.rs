//! Approval-bound exports from the wallet's existing stock. No consume, flush, signing or
//! transfer-record creation occurs here. Callers must sync the wallet before preparation.

use super::*;
use bdk_wallet::bitcoin::Transaction as BitcoinTransaction;
use rgbstd::{Operation as _, TypedAssigns};
use rust_only::ColoringInfo;

/// Semantic destination of one fungible allocation. Fee policy belongs to the caller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AllocationRole {
    /// Principal recipient.
    Principal,
    /// Asset change recipient.
    Change,
    /// Maker fee recipient.
    MakerFee,
    /// Taker fee recipient.
    TakerFee,
}

/// Explicit consent for one revealed allocation in the approved transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedAllocation {
    /// Contract identifier.
    pub contract_id: ContractId,
    /// Transition identifier, as displayed by the engine.
    pub transition_id: String,
    /// Semantic destination.
    pub role: AllocationRole,
    /// Vout in the final colored transaction (including any inserted commitment output).
    pub vout: u32,
    /// Exact recipient script.
    pub script_pubkey: ScriptBuf,
    /// Exact fungible amount. Zero fee legs are omitted, never represented as allocations.
    pub amount: u64,
    /// Require this wallet's derivation registry to own this recipient (always required for change).
    pub require_local_ownership: bool,
}

/// Consent must come from the approving application, after inspecting the candidate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProspectiveApproval {
    /// Exact serialized colored PSBT, including transition metadata.
    pub psbt: Vec<u8>,
    /// Exact engine-produced fascia JSON.
    pub fascia_json: String,
    /// Complete allocation list; extra, missing or duplicate allocations are refused.
    pub allocations: Vec<ApprovedAllocation>,
}

/// Opaque, unsigned candidate constructed by this wallet. Approval does not mutate it.
pub struct ProspectiveTransfer {
    psbt: Psbt,
    fascia: Fascia,
}

impl ProspectiveTransfer {
    /// Final colored PSBT to inspect and approve.
    pub fn psbt(&self) -> &Psbt {
        &self.psbt
    }
    /// Engine-produced prospective fascia to inspect and approve.
    pub fn fascia(&self) -> &Fascia {
        &self.fascia
    }
}

fn refusal(details: impl Into<String>) -> Error {
    Error::RgbInspection {
        details: details.into(),
    }
}

fn check_unsigned(psbt: &Psbt) -> Result<(), Error> {
    if psbt.inputs.len() != psbt.unsigned_tx.input.len()
        || psbt.outputs.len() != psbt.unsigned_tx.output.len()
        || psbt.inputs.iter().any(|i| {
            !i.partial_sigs.is_empty()
                || i.tap_key_sig.is_some()
                || !i.tap_script_sigs.is_empty()
                || i.final_script_sig.is_some()
                || i.final_script_witness.is_some()
        })
        || psbt
            .unsigned_tx
            .input
            .iter()
            .any(|i| !i.script_sig.is_empty() || !i.witness.is_empty())
    {
        return Err(refusal(
            "prospective PSBT must be unsigned and structurally complete",
        ));
    }
    Ok(())
}

/// Resolve every prevout from an independent source. PSBT amounts/scripts are assertions,
/// never the source of truth. A resolver must authenticate its transaction source/network.
fn resolve_inputs<F>(psbt: &Psbt, mut resolve: F) -> Result<Vec<(OutPoint, TxOut)>, Error>
where
    F: FnMut(OutPoint) -> Result<BitcoinTransaction, Error>,
{
    check_unsigned(psbt)?;
    let mut seen = HashSet::new();
    psbt.unsigned_tx
        .input
        .iter()
        .zip(&psbt.inputs)
        .map(|(txin, input)| {
            let outpoint = txin.previous_output;
            if !seen.insert(outpoint) {
                return Err(refusal("duplicate input"));
            }
            let previous = resolve(outpoint)?;
            if previous.compute_txid() != outpoint.txid {
                return Err(refusal("resolved prevout txid mismatch"));
            }
            let output = previous
                .output
                .get(outpoint.vout as usize)
                .ok_or_else(|| refusal("resolved prevout vout missing"))?
                .clone();
            if input.witness_utxo.as_ref() != Some(&output)
                || input
                    .non_witness_utxo
                    .as_ref()
                    .is_some_and(|tx| tx != &previous)
            {
                return Err(refusal(
                    "PSBT prevout amount/script differs from resolved transaction",
                ));
            }
            Ok((outpoint, output))
        })
        .collect()
}

fn check_approval(
    candidate: &ProspectiveTransfer,
    approval: &ProspectiveApproval,
) -> Result<(), Error> {
    check_unsigned(&candidate.psbt)?;
    if candidate.psbt.serialize() != approval.psbt
        || serde_json::to_string(&candidate.fascia).map_err(InternalError::from)?
            != approval.fascia_json
    {
        return Err(refusal("candidate differs from exact colored approval"));
    }
    if candidate.fascia.witness_id().to_string()
        != candidate.psbt.unsigned_tx.compute_txid().to_string()
    {
        return Err(refusal("fascia witness differs from approved transaction"));
    }
    let mut actual = BTreeMap::new();
    for (contract, bundle) in candidate.fascia.bundles() {
        if bundle.known_transitions.len() != 1 {
            return Err(refusal("exactly one transition per contract required"));
        }
        let transition = &bundle
            .known_transitions
            .first()
            .expect("length checked")
            .transition;
        if transition.transition_type != TS_TRANSFER {
            return Err(refusal("only transfer transitions supported"));
        }
        for (ty, assignments) in transition.assignments.iter() {
            if *ty != OS_ASSET {
                return Err(refusal("non-asset assignment refused"));
            }
            let TypedAssigns::Fungible(assignments) = assignments else {
                return Err(refusal("only fungible allocations supported"));
            };
            for assignment in assignments.iter() {
                let Assign::Revealed { seal, state, .. } = assignment else {
                    return Err(refusal("concealed allocation refused"));
                };
                if seal.txid != TxPtr::WitnessTx {
                    return Err(refusal("allocation is not in approved witness"));
                }
                let vout = seal.vout.into_u32();
                let output = candidate
                    .psbt
                    .unsigned_tx
                    .output
                    .get(vout as usize)
                    .ok_or_else(|| refusal("allocation vout missing"))?;
                let key = (*contract, transition.id().to_string(), vout);
                if state.as_u64() == 0
                    || actual
                        .insert(key, (output.script_pubkey.clone(), state.as_u64()))
                        .is_some()
                {
                    return Err(refusal("zero or duplicate allocation"));
                }
            }
        }
    }
    let mut expected = BTreeMap::new();
    let mut roles = HashSet::new();
    for allocation in &approval.allocations {
        if !roles.insert((allocation.contract_id, allocation.role))
            || expected
                .insert(
                    (
                        allocation.contract_id,
                        allocation.transition_id.clone(),
                        allocation.vout,
                    ),
                    (allocation.script_pubkey.clone(), allocation.amount),
                )
                .is_some()
        {
            return Err(refusal("duplicate approved role/allocation"));
        }
    }
    if candidate
        .fascia
        .bundles()
        .keys()
        .any(|contract| !roles.contains(&(*contract, AllocationRole::Principal)))
    {
        return Err(refusal(
            "each transferred asset requires an approved principal",
        ));
    }
    if actual.is_empty() || actual != expected {
        return Err(refusal(
            "approved asset/recipient/allocation/amount mismatch",
        ));
    }
    Ok(())
}

impl Wallet {
    /// Construct a prospective transfer on a clone, using only existing stock under its wallet
    /// lock. This neither initializes RGB state nor persists it on success, failure or drop.
    /// Resolve all local and foreign Bitcoin inputs independently; only locally unspent inputs
    /// may contribute RGB state. No network sync is performed by this method.
    pub fn prepare_prospective_transfer<F>(
        &self,
        psbt: &Psbt,
        coloring: ColoringInfo,
        resolve: F,
    ) -> Result<ProspectiveTransfer, Error>
    where
        F: FnMut(OutPoint) -> Result<BitcoinTransaction, Error>,
    {
        for asset in coloring.asset_info_map.values() {
            if asset.output_map.is_empty()
                || asset.output_map.iter().any(|(vout, amount)| {
                    *amount == 0 || *vout as usize >= psbt.unsigned_tx.output.len()
                })
                || asset
                    .output_map
                    .values()
                    .try_fold(0u64, |sum, amount| sum.checked_add(*amount))
                    .is_none()
            {
                return Err(refusal("invalid or overflowing prospective coloring"));
            }
        }
        let inputs = resolve_inputs(psbt, resolve)?;
        let local = self
            .bdk_wallet()
            .list_unspent()
            .map(|u| (u.outpoint, u.txout))
            .collect::<HashMap<_, _>>();
        let owned = inputs
            .iter()
            .filter_map(|(outpoint, output)| {
                (local.get(outpoint) == Some(output)).then_some(*outpoint)
            })
            .collect::<Vec<_>>();
        if owned.is_empty() {
            return Err(refusal("no authenticated local unspent input"));
        }
        // Check foreign inputs cannot supply RGB allocations known in our stock.
        crate::utils::with_rgb_runtime_readonly(self.wallet_dir(), |runtime| {
            if coloring.asset_info_map.is_empty() {
                return Err(refusal("no asset coloring supplied"));
            }
            for contract in coloring.asset_info_map.keys() {
                if AssetSchema::get_from_contract_id(*contract, runtime)? == AssetSchema::Uda {
                    return Err(refusal("prospective export supports fungible assets only"));
                }
                let foreign = inputs
                    .iter()
                    .filter_map(|(outpoint, _)| (!owned.contains(outpoint)).then_some(*outpoint));
                if !runtime
                    .contract_assignments_for(*contract, foreign)?
                    .is_empty()
                {
                    return Err(refusal("foreign input supplies RGB state"));
                }
            }
            let mut colored = psbt.clone();
            let (fascia, _) = self.color_psbt_with_runtime(&mut colored, coloring, runtime)?;
            Ok(ProspectiveTransfer {
                psbt: colored,
                fascia,
            })
        })
    }

    /// Export all approved recipients from the SAME engine-built transaction/transition,
    /// without consuming its fascia or creating durable transfer records. Re-resolve inputs
    /// and recheck stock ownership after approval; a stale or changed state is refused.
    pub fn export_prospective_transfer<F>(
        &self,
        candidate: &ProspectiveTransfer,
        approval: &ProspectiveApproval,
        resolve: F,
    ) -> Result<Vec<RgbTransfer>, Error>
    where
        F: FnMut(OutPoint) -> Result<BitcoinTransaction, Error>,
    {
        check_approval(candidate, approval)?;
        for allocation in &approval.allocations {
            if (allocation.require_local_ownership || allocation.role == AllocationRole::Change)
                && !self.bdk_wallet().is_mine(allocation.script_pubkey.clone())
            {
                return Err(refusal(
                    "approved local recipient is not owned by this wallet",
                ));
            }
        }
        let inputs = resolve_inputs(&candidate.psbt, resolve)?;
        let local = self
            .bdk_wallet()
            .list_unspent()
            .map(|u| (u.outpoint, u.txout))
            .collect::<HashMap<_, _>>();
        let owned = inputs
            .iter()
            .filter_map(|(p, o)| (local.get(p) == Some(o)).then_some(*p))
            .collect::<Vec<_>>();
        if owned.is_empty() {
            return Err(refusal("local inputs are no longer unspent"));
        }
        crate::utils::with_rgb_runtime_readonly(self.wallet_dir(), |runtime| {
            let mut transfers = Vec::new();
            for (contract, bundle) in candidate.fascia.bundles() {
                let assignments =
                    runtime.contract_assignments_for(*contract, owned.iter().copied())?;
                let mut available = 0u64;
                for states in assignments.values() {
                    for (opout, state) in states {
                        let AllocatedState::Amount(amount) = state else {
                            return Err(refusal("non-fungible input state refused"));
                        };
                        if opout.ty != OS_ASSET {
                            return Err(refusal("non-asset input state refused"));
                        }
                        available = available
                            .checked_add(amount.as_u64())
                            .ok_or_else(|| refusal("input amount overflow"))?;
                    }
                }
                let allocated = approval
                    .allocations
                    .iter()
                    .filter(|a| a.contract_id == *contract)
                    .try_fold(0u64, |sum, a| sum.checked_add(a.amount))
                    .ok_or_else(|| refusal("allocation amount overflow"))?;
                if allocated != available {
                    return Err(refusal(
                        "all local asset input value must be allocated explicitly",
                    ));
                }
                let stash_inputs = assignments
                    .into_values()
                    .flat_map(|a| a.into_keys())
                    .collect::<HashSet<_>>();
                let transition_inputs = bundle
                    .known_transitions
                    .iter()
                    .flat_map(|kt| kt.transition.inputs())
                    .collect::<HashSet<_>>();
                if stash_inputs.is_empty() || stash_inputs != transition_inputs {
                    return Err(refusal("RGB inputs differ from local stock"));
                }
                let outputs = approval
                    .allocations
                    .iter()
                    .filter(|a| a.contract_id == *contract)
                    .map(|a| ExplicitSeal::with(candidate.fascia.witness_id(), a.vout))
                    .collect::<Vec<_>>();
                transfers.push(runtime.transfer_from_fascia(
                    *contract,
                    outputs,
                    Vec::<SecretSeal>::new(),
                    &candidate.fascia,
                )?);
            }
            Ok(transfers)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amplify::ByteArray;
    use bdk_wallet::bitcoin::{Sequence, TxIn, Witness, absolute, transaction};
    use rgbstd::OpId;

    fn transaction(outputs: Vec<TxOut>) -> BitcoinTransaction {
        BitcoinTransaction {
            version: transaction::Version::TWO,
            lock_time: absolute::LockTime::ZERO,
            input: vec![TxIn {
                previous_output: OutPoint::null(),
                script_sig: ScriptBuf::new(),
                sequence: Sequence::MAX,
                witness: Witness::new(),
            }],
            output: outputs,
        }
    }

    // Synthetic engine-built transition/commitment. No wallet, issued contract, network or funds.
    fn candidate(
        maker: u64,
        taker: u64,
    ) -> (ProspectiveTransfer, ProspectiveApproval, BitcoinTransaction) {
        let contract = ContractId::from_byte_array([1; 32]);
        let opout = Opout::new(OpId::from_byte_array([2; 32]), OS_ASSET, 0);
        let amounts = [
            (AllocationRole::Principal, 1000),
            (AllocationRole::Change, 100),
            (AllocationRole::MakerFee, maker),
            (AllocationRole::TakerFee, taker),
        ];
        let previous = transaction(vec![TxOut {
            value: BdkAmount::from_sat(10000),
            script_pubkey: ScriptBuf::from_bytes(vec![0x51]),
        }]);
        let mut tx = transaction(vec![TxOut {
            value: BdkAmount::ZERO,
            script_pubkey: ScriptBuf::new_op_return([]),
        }]);
        tx.input[0].previous_output = OutPoint::new(previous.compute_txid(), 0);
        let mut builder = TransitionBuilder::with(
            contract,
            NonInflatableAsset::schema(),
            TS_TRANSFER,
            NonInflatableAsset::types(),
        )
        .add_input(
            opout,
            AllocatedState::Amount(Amount::from(1100 + maker + taker).into()),
        )
        .unwrap();
        let mut allocations = vec![];
        for (role, amount) in amounts {
            if amount == 0 {
                continue;
            }
            let vout = tx.output.len() as u32;
            let script = ScriptBuf::from_bytes(vec![0x51 + vout as u8]);
            tx.output.push(TxOut {
                value: BdkAmount::from_sat(330),
                script_pubkey: script.clone(),
            });
            builder = builder
                .add_fungible_state_raw(OS_ASSET, GraphSeal::with_blinded_vout(vout, 777), amount)
                .unwrap();
            allocations.push(ApprovedAllocation {
                contract_id: contract,
                transition_id: String::new(),
                role,
                vout,
                script_pubkey: script,
                amount,
                require_local_ownership: false,
            });
        }
        let transition = builder.set_nonce(1).complete_transition().unwrap();
        for allocation in &mut allocations {
            allocation.transition_id = transition.id().to_string();
        }
        let mut psbt = Psbt::from_unsigned_tx(tx).unwrap();
        psbt.inputs[0].witness_utxo = Some(previous.output[0].clone());
        psbt.set_rgb_contract_consumer(contract, opout, transition.id())
            .unwrap();
        psbt.push_rgb_transition(transition).unwrap();
        psbt.outputs[0].set_opret_host();
        psbt.outputs[0].set_mpc_entropy(777).unwrap();
        psbt.set_rgb_close_method(CloseMethod::OpretFirst);
        psbt.set_as_unmodifiable();
        let fascia = psbt.rgb_commit().unwrap();
        let approval = ProspectiveApproval {
            psbt: psbt.serialize(),
            fascia_json: serde_json::to_string(&fascia).unwrap(),
            allocations,
        };
        (ProspectiveTransfer { psbt, fascia }, approval, previous)
    }

    #[test]
    fn prospective_commitment_insertion_preserves_metadata_and_existing_vouts() {
        let (candidate, _, _) = candidate(0, 0);
        let mut psbt = candidate.psbt.clone();
        psbt.unsigned_tx.output.remove(0);
        psbt.outputs.remove(0);
        let mut p2tr = vec![0x51, 0x20];
        p2tr.extend_from_slice(&[1; 32]);
        psbt.unsigned_tx.output[0].script_pubkey = ScriptBuf::from_bytes(p2tr);
        psbt.inputs[0].unknown.insert(
            bdk_wallet::bitcoin::psbt::raw::Key {
                type_value: 0x80,
                key: vec![1],
            },
            vec![2],
        );
        psbt.outputs[0].unknown.insert(
            bdk_wallet::bitcoin::psbt::raw::Key {
                type_value: 0x80,
                key: vec![3],
            },
            vec![4],
        );
        let inputs_before = psbt.inputs.clone();
        let outputs_before = psbt.outputs.clone();
        assert!(rust_only::prepare_commitment_output(&mut psbt).unwrap());
        assert_eq!(psbt.inputs, inputs_before);
        assert_eq!(&psbt.outputs[1..], outputs_before.as_slice());
        let existing = psbt.clone();
        assert!(!rust_only::prepare_commitment_output(&mut psbt).unwrap());
        assert_eq!(psbt, existing);
        // Without Taproot the host is appended and recipient vouts do not shift.
        psbt.unsigned_tx.output.remove(0);
        psbt.outputs.remove(0);
        psbt.unsigned_tx.output[0].script_pubkey = ScriptBuf::from_bytes(vec![0x51]);
        let output_before = psbt.unsigned_tx.output[0].clone();
        assert!(!rust_only::prepare_commitment_output(&mut psbt).unwrap());
        assert_eq!(psbt.unsigned_tx.output[0], output_before);
        assert_eq!(psbt.inputs, inputs_before);
    }

    #[test]
    fn prospective_zero_maker_only_and_both_fees_bind_all_allocations() {
        for (maker, taker, count) in [(0, 0, 2), (5, 0, 3), (5, 2, 4)] {
            let (candidate, approval, previous) = candidate(maker, taker);
            assert_eq!(approval.allocations.len(), count);
            check_approval(&candidate, &approval).unwrap();
            resolve_inputs(&candidate.psbt, |_| Ok(previous.clone())).unwrap();
        }
    }

    #[test]
    fn prospective_wrong_recipient_asset_transition_amount_and_missing_extra_refused() {
        let (candidate, approval, _) = candidate(5, 2);
        for field in 0..8 {
            let mut wrong = approval.clone();
            match field {
                0 => wrong.allocations[2].script_pubkey = ScriptBuf::new(),
                1 => wrong.allocations[2].contract_id = ContractId::from_byte_array([3; 32]),
                2 => wrong.allocations[2].transition_id = "wrong".into(),
                3 => wrong.allocations[2].amount += 1,
                4 => {
                    wrong.allocations.remove(2);
                }
                5 => wrong.allocations.push(wrong.allocations[2].clone()),
                6 => wrong.allocations[2].vout = 99,
                _ => wrong.allocations[2].role = AllocationRole::Principal,
            }
            assert!(check_approval(&candidate, &wrong).is_err(), "field {field}");
        }
    }

    #[test]
    fn prospective_post_approval_psbt_fascia_and_signature_refused() {
        let (mut candidate, approval, _) = candidate(5, 2);
        candidate.psbt.unsigned_tx.output[1].value = BdkAmount::from_sat(331);
        assert!(check_approval(&candidate, &approval).is_err());
        let (mut candidate, approval, _) = self::candidate(5, 2);
        candidate
            .fascia
            .update_pub_witness(rgbstd::containers::PubWitness::with(transaction(vec![])));
        assert!(check_approval(&candidate, &approval).is_err());
        let (mut candidate, mut approval, _) = self::candidate(5, 2);
        candidate.psbt.inputs[0].final_script_witness = Some(Witness::new());
        approval.psbt = candidate.psbt.serialize();
        assert!(check_approval(&candidate, &approval).is_err());
    }

    #[test]
    fn prospective_prevout_amount_script_txid_vout_and_duplicate_refused() {
        let (candidate, _, previous) = candidate(5, 2);
        let mut wrong = candidate.psbt.clone();
        wrong.inputs[0].witness_utxo.as_mut().unwrap().value = BdkAmount::from_sat(9999);
        assert!(resolve_inputs(&wrong, |_| Ok(previous.clone())).is_err());
        wrong.inputs[0].witness_utxo = Some(previous.output[0].clone());
        wrong.inputs[0].witness_utxo.as_mut().unwrap().script_pubkey = ScriptBuf::new();
        assert!(resolve_inputs(&wrong, |_| Ok(previous.clone())).is_err());
        assert!(resolve_inputs(&candidate.psbt, |_| Ok(transaction(vec![]))).is_err());
        wrong = candidate.psbt.clone();
        wrong.unsigned_tx.input[0].previous_output.vout = 99;
        assert!(resolve_inputs(&wrong, |_| Ok(previous.clone())).is_err());
        wrong = candidate.psbt.clone();
        wrong
            .unsigned_tx
            .input
            .push(wrong.unsigned_tx.input[0].clone());
        wrong.inputs.push(wrong.inputs[0].clone());
        assert!(resolve_inputs(&wrong, |_| Ok(previous.clone())).is_err());
    }
}
