use std::{str::FromStr, sync::Arc};

use ethers::{
	providers::{JsonRpcClient, Provider},
	types::{Address, Signature, TransactionRequest, H160, U64},
};

use crate::{
	authority::AuthorityContract, chainlink_aggregator::ChainlinkContract,
	relayer_manager::RelayerManagerContract, socket::SocketContract, vault::VaultContract,
	INVALID_CONTRACT_ADDRESS,
};

/// The type of EVM chain ID's.
pub type ChainID = u32;

/// The native chain's average block time in seconds.
pub const NATIVE_BLOCK_TIME: u32 = 3u32;

/// Ethereum network's average block time in seconds.
pub const ETHEREUM_BLOCK_TIME: u64 = 12u64;

/// The block range chunk size for getLogs requests.
pub const BOOTSTRAP_BLOCK_CHUNK_SIZE: u64 = 2000;

/// The block offset used to measure the average block time at bootstrap.
pub const BOOTSTRAP_BLOCK_OFFSET: u32 = 100;

/// The metadata of the EVM provider.
pub struct ProviderMetadata {
	pub name: String,
	/// Id of chain which this client interact with.
	pub id: ChainID,
	/// The total number of confirmations required for a block to be processed. (block
	/// confirmations + eth_getLogs batch size)
	pub block_confirmations: U64,
	/// The batch size used on `eth_getLogs()` requests.
	pub get_logs_batch_size: U64,
	/// The `get_block` request interval in milliseconds.
	pub call_interval: u64,
	/// Bridge direction when bridge event points this chain as destination.
	pub if_destination_chain: BridgeDirection,
	/// The flag whether the chain is Bifrost(native) or an external chain.
	pub is_native: bool,
}

impl ProviderMetadata {
	pub fn new(
		name: String,
		id: ChainID,
		block_confirmations: u64,
		call_interval: u64,
		get_logs_batch_size: u64,
		is_native: bool,
	) -> Self {
		Self {
			name,
			id,
			block_confirmations: U64::from(block_confirmations.saturating_add(get_logs_batch_size)),
			get_logs_batch_size: U64::from(get_logs_batch_size),
			call_interval,
			is_native,
			if_destination_chain: match is_native {
				true => BridgeDirection::Inbound,
				false => BridgeDirection::Outbound,
			},
		}
	}
}

pub struct AggregatorContracts<T> {
	/// Chainlink usdc/usd aggregator
	pub chainlink_usdc_usd: Option<ChainlinkContract<Provider<T>>>,
	/// Chainlink usdt/usd aggregator
	pub chainlink_usdt_usd: Option<ChainlinkContract<Provider<T>>>,
	/// Chainlink dai/usd aggregator
	pub chainlink_dai_usd: Option<ChainlinkContract<Provider<T>>>,
}

impl<T: JsonRpcClient> AggregatorContracts<T> {
	pub fn new(
		provider: Arc<Provider<T>>,
		chainlink_usdc_usd_address: Option<String>,
		chainlink_usdt_usd_address: Option<String>,
		chainlink_dai_usd_address: Option<String>,
	) -> Self {
		Self {
			chainlink_usdc_usd: chainlink_usdc_usd_address.map(|address| {
				ChainlinkContract::new(
					H160::from_str(&address).expect(INVALID_CONTRACT_ADDRESS),
					provider.clone(),
				)
			}),
			chainlink_usdt_usd: chainlink_usdt_usd_address.map(|address| {
				ChainlinkContract::new(
					H160::from_str(&address).expect(INVALID_CONTRACT_ADDRESS),
					provider.clone(),
				)
			}),
			chainlink_dai_usd: chainlink_dai_usd_address.map(|address| {
				ChainlinkContract::new(
					H160::from_str(&address).expect(INVALID_CONTRACT_ADDRESS),
					provider.clone(),
				)
			}),
		}
	}
}

/// The protocol contract instances of the EVM provider.
pub struct ProtocolContracts<T> {
	/// SocketContract
	pub socket: SocketContract<Provider<T>>,
	/// VaultContract
	pub vault: VaultContract<Provider<T>>,
	/// AuthorityContract
	pub authority: AuthorityContract<Provider<T>>,
	/// RelayerManagerContract (Bifrost only)
	pub relayer_manager: Option<RelayerManagerContract<Provider<T>>>,
}

impl<T: JsonRpcClient> ProtocolContracts<T> {
	pub fn new(
		provider: Arc<Provider<T>>,
		socket_address: String,
		vault_address: String,
		authority_address: String,
		relayer_manager_address: Option<String>,
	) -> Self {
		Self {
			socket: SocketContract::new(
				H160::from_str(&socket_address).expect(INVALID_CONTRACT_ADDRESS),
				provider.clone(),
			),
			vault: VaultContract::new(
				H160::from_str(&vault_address).expect(INVALID_CONTRACT_ADDRESS),
				provider.clone(),
			),
			authority: AuthorityContract::new(
				H160::from_str(&authority_address).expect(INVALID_CONTRACT_ADDRESS),
				provider.clone(),
			),
			relayer_manager: relayer_manager_address.map(|address| {
				RelayerManagerContract::new(
					H160::from_str(&address).expect(INVALID_CONTRACT_ADDRESS),
					provider.clone(),
				)
			}),
		}
	}
}

#[derive(Clone, Debug)]
/// Coefficients to multiply the estimated gas amount.
pub enum GasCoefficient {
	/// The lowest coefficient. Only used on transaction submissions to external chains.
	Low,
	/// The medium coefficient. Only used on transaction submissions to Bifrost.
	Mid,
	/// The high coefficient. Currently not in used.
	High,
}

impl GasCoefficient {
	pub fn into_f64(&self) -> f64 {
		match self {
			GasCoefficient::Low => 1.2,
			GasCoefficient::Mid => 7.0,
			GasCoefficient::High => 10.0,
		}
	}
}

#[derive(Clone, Copy, Debug)]
/// The roundup event status.
pub enum RoundUpEventStatus {
	/// A single relayer has relayed a `RoundUp` event, however the quorum wasn't reached yet.
	NextAuthorityRelayed = 9,
	/// A single relayer has relayed a `RoundUp` event and the quorum has been reached.
	NextAuthorityCommitted,
}

impl RoundUpEventStatus {
	pub fn from_u8(status: u8) -> Self {
		match status {
			9 => RoundUpEventStatus::NextAuthorityRelayed,
			10 => RoundUpEventStatus::NextAuthorityCommitted,
			_ => panic!("Unknown roundup event status received: {:?}", status),
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
/// The socket event status.
pub enum SocketEventStatus {
	Requested = 1,
	Failed,
	Executed,
	Reverted,
	Accepted,
	Rejected,
	Committed,
	Rollbacked,
}

impl SocketEventStatus {
	pub fn from_u8(status: u8) -> Self {
		match status {
			1 => SocketEventStatus::Requested,
			2 => SocketEventStatus::Failed,
			3 => SocketEventStatus::Executed,
			4 => SocketEventStatus::Reverted,
			5 => SocketEventStatus::Accepted,
			6 => SocketEventStatus::Rejected,
			7 => SocketEventStatus::Committed,
			8 => SocketEventStatus::Rollbacked,
			_ => panic!("Unknown socket event status received: {:?}", status),
		}
	}
}

impl From<SocketEventStatus> for u8 {
	fn from(status: SocketEventStatus) -> Self {
		match status {
			SocketEventStatus::Requested => 1,
			SocketEventStatus::Failed => 2,
			SocketEventStatus::Executed => 3,
			SocketEventStatus::Reverted => 4,
			SocketEventStatus::Accepted => 5,
			SocketEventStatus::Rejected => 6,
			SocketEventStatus::Committed => 7,
			SocketEventStatus::Rollbacked => 8,
		}
	}
}

#[derive(Clone, Copy, Debug)]
/// The CCCP protocols bridge direction.
pub enum BridgeDirection {
	/// From external network, to bifrost network.
	Inbound,
	/// From bifrost network, to external network.
	Outbound,
}

#[derive(Clone, Debug, PartialEq)]
/// The state for bootstrapping
pub enum BootstrapState {
	/// phase 0. check if the node is in syncing
	NodeSyncing,
	/// phase 1-1. emit all pushed RoundUp event
	BootstrapRoundUpPhase1,
	/// phase 1-2. bootstrap for RoundUp event
	BootstrapRoundUpPhase2,
	/// phase 2. bootstrap for Bridge event
	BootstrapBridgeRelay,
	/// phase 3. process for latest block as normal
	NormalStart,
}

#[derive(Clone, Debug)]
/// The information of a recovered signature.
pub struct RecoveredSignature {
	/// The original index that represents the order from the result of `get_signatures()`.
	pub idx: usize,
	/// The signature of the message.
	pub signature: Signature,
	/// The account who signed the message.
	pub signer: Address,
}

impl RecoveredSignature {
	pub fn new(idx: usize, signature: Signature, signer: Address) -> Self {
		Self { idx, signature, signer }
	}
}

#[derive(Clone, Debug)]
/// The built relay transaction request.
pub struct BuiltRelayTransaction {
	/// The raw transaction request body.
	pub tx_request: TransactionRequest,
	/// The flag whether if the destination is an external chain.
	pub is_external: bool,
}

impl BuiltRelayTransaction {
	pub fn new(tx_request: TransactionRequest, is_external: bool) -> Self {
		Self { tx_request, is_external }
	}
}
