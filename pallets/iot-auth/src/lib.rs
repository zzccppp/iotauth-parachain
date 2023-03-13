#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

use cumulus_pallet_xcm::{ensure_sibling_para, Origin as CumulusOrigin};
use cumulus_primitives_core::ParaId;
use frame_support::{parameter_types, BoundedVec};
use frame_system::Config as SystemConfig;
use sp_runtime::traits::Saturating;
use sp_std::prelude::*;
use xcm::latest::prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::Randomness;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		// type Currency: Currency<Self::AccountId>;
		type CollectionRandomness: Randomness<Self::Hash, Self::BlockNumber>;

		#[pallet::constant]
		type MaximumOwned: Get<u32>;

		#[pallet::constant]
		type MaxDeviceDescription: Get<u32>;

		#[pallet::constant]
		type MaxCredentialLength: Get<u32>;

		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type RuntimeOrigin: From<<Self as SystemConfig>::RuntimeOrigin>
			+ Into<Result<CumulusOrigin, <Self as Config>::RuntimeOrigin>>;
		/// The overarching call type; we assume sibling chains use the same type.
		type RuntimeCall: From<Call<Self>> + Encode;
		type XcmSender: SendXcm;
		type SelfParaId: Get<ParaId>;
	}

	#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
	#[scale_info(skip_type_params(T))]
	pub struct IotDevice<T: Config> {
		pub unique_id: [u8; 16],
		pub owner: T::AccountId,
		pub description: BoundedVec<u8, T::MaxDeviceDescription>, // 设备的信息
		pub hardware_id: [u8; 16],                                // 设备的硬件ID
		                                                          // May Add More Later
		                                                          // 设备的公钥
	}

	#[pallet::storage]
	pub(super) type IotDeviceCount<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	pub(super) type IotDeviceMap<T: Config> = StorageMap<_, Twox64Concat, [u8; 16], IotDevice<T>>;

	#[pallet::storage]
	pub(super) type OwnerOfCollectibles<T: Config> = StorageMap<
		_,
		Twox64Concat,
		T::AccountId,
		BoundedVec<[u8; 16], T::MaximumOwned>,
		ValueQuery,
	>;

	#[pallet::storage]
	pub(super) type CAPublicKey<T: Config> = StorageValue<_, [u8; 32], ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		/// Each description must have a unique identifier
		DuplicateDevices,
		/// An account can't exceed the `MaximumOwned` constant
		MaximumDevicesOwned,
		/// The total supply of description can't exceed the u64 limit
		BoundsOverflow,
		/// The device description is too long
		DescriptionTooLong,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A new collectible was successfully created.
		DeviceCreated {
			collectible: [u8; 16],
			owner: T::AccountId,
		},
		CAPublicKeyUpdated {
			public_key: [u8; 32],
		},
		LocalAuthSuccess {
			account: T::AccountId,
			unique_id: [u8; 16],
		},
		LocalAuthFailed {
			account: T::AccountId,
		},
		LocalAuthCredentialIssued {
			credential: BoundedVec<u8, T::MaxCredentialLength>,
			signature: [u8; 64],
		},
		ParaAuthSuccess {
			account: T::AccountId,
			unique_id: [u8; 16],
			para_source: ParaId,
			para_remote: ParaId,
		},
		ParaAuthFailed {
			account: T::AccountId,
			para_source: ParaId,
		},
		ParaAuthCredentialIssued {
			credential: BoundedVec<u8, T::MaxCredentialLength>,
			signature: [u8; 64],
			para_source: ParaId,
			para_remote: ParaId,
		},
		TestForQueryId {
			parachain_id: ParaId,
		},
		TestMessage {
			message: u32,
		},
		TestResponse {
			unique_id: [u8; 16],
			account: T::AccountId,
		},
	}

	impl<T: Config> Pallet<T> {
		fn gen_unique_id() -> [u8; 16] {
			// Create randomness
			let random = T::CollectionRandomness::random(&b"unique_id"[..]).0;

			// Create randomness payload. Multiple collectibles can be generated in the same block,
			// retaining uniqueness.
			let unique_payload = (
				random,
				frame_system::Pallet::<T>::extrinsic_index().unwrap_or_default(),
				frame_system::Pallet::<T>::block_number(),
			);

			// Turns into a byte array
			let encoded_payload = unique_payload.encode();
			let hash = frame_support::Hashable::blake2_128(&encoded_payload);

			hash
		}

		// Function to mint a collectible
		pub fn mint(
			owner: &T::AccountId,
			unique_id: [u8; 16],
			desc: &Vec<u8>,
			hardware_id: [u8; 16],
		) -> Result<[u8; 16], DispatchError> {
			// Create a new object
			let collectible = IotDevice::<T> {
				unique_id,
				owner: owner.clone(),
				description: BoundedVec::try_from(desc.clone())
					.map_err(|_| Error::<T>::DescriptionTooLong)?,
				hardware_id,
			};

			// Check if the collectible exists in the storage map
			ensure!(
				!IotDeviceMap::<T>::contains_key(&collectible.unique_id),
				Error::<T>::DuplicateDevices
			);

			// Check that a new collectible can be created
			let count = IotDeviceCount::<T>::get();
			let new_count = count.checked_add(1).ok_or(Error::<T>::BoundsOverflow)?;

			// Append collectible to OwnerOfCollectibles map
			OwnerOfCollectibles::<T>::try_append(&owner, collectible.unique_id)
				.map_err(|_| Error::<T>::MaximumDevicesOwned)?;

			// Write new collectible to storage and update the count
			IotDeviceMap::<T>::insert(collectible.unique_id, collectible);
			IotDeviceCount::<T>::put(new_count);

			// Deposit the "Collectiblereated" event.
			Self::deposit_event(Event::DeviceCreated {
				collectible: unique_id,
				owner: owner.clone(),
			});

			// Returns the unique_id of the new collectible if this succeeds
			Ok(unique_id)
		}
	}

	// Pallet callable functions
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new unique devices
		#[pallet::call_index(1)]
		#[pallet::weight(0)]
		pub fn create_devices(
			origin: OriginFor<T>,
			description: Vec<u8>,
			hardware_id: [u8; 16],
		) -> DispatchResult {
			// Make sure the caller is from a signed origin
			let sender = ensure_signed(origin)?;

			// Generate the unique_id and color using a helper function
			let collectible_gen_unique_id = Self::gen_unique_id();

			// Write new collectible to storage by calling helper function
			Self::mint(&sender, collectible_gen_unique_id, &description, hardware_id)?;

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(0)]
		pub fn query_id(origin: OriginFor<T>) -> DispatchResult {
			let sender = ensure_signed(origin)?;
			let parachain_id = T::SelfParaId::get();
			Self::deposit_event(Event::TestForQueryId { parachain_id });
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(0)]
		pub fn request_for_parachain_auth(
			origin: OriginFor<T>,
			unique_id: [u8; 16],
			para: ParaId,
		) -> DispatchResult {
			// will be executed in the local parachain (A)
			let sender = ensure_signed(origin)?;

			match T::XcmSender::send_xcm(
				(1, Junction::Parachain(para.into())),
				Xcm(vec![Transact {
					origin_type: OriginKind::Native,
					require_weight_at_most: 1_000,
					call: <T as Config>::RuntimeCall::from(Call::<T>::reponse_for_parachain_auth {
						sender,
						unique_id,
					})
					.encode()
					.into(),
				}]),
			) {
				Ok(()) => {
					Self::deposit_event(Event::TestMessage { message: 11 });
				},
				Err(e) => {
					Self::deposit_event(Event::TestMessage { message: 22 });
				},
			};

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(0)]
		pub fn reponse_for_parachain_auth(
			origin: OriginFor<T>,
			sender: T::AccountId,
			unique_id: [u8; 16],
		) -> DispatchResult {
			// will be executed in the parachain (B)
			let para_source = ensure_sibling_para(<T as Config>::RuntimeOrigin::from(origin))?;
			let para_local = T::SelfParaId::get();

			// 保证该用户存在
			if !OwnerOfCollectibles::<T>::contains_key(&sender) {
				Self::deposit_event(Event::ParaAuthFailed { account: sender.clone(), para_source });

				return Ok(());
			}

			// 保证该用户拥有该设备
			match OwnerOfCollectibles::<T>::get(&sender).iter().find(|&&x| x == unique_id) {
				Some(dev) => {
					Self::deposit_event(Event::ParaAuthSuccess {
						account: sender,
						unique_id: dev.clone(),
						para_source,
						para_remote: para_local,
					});
				},
				None => {
					Self::deposit_event(Event::ParaAuthFailed {
						account: sender.clone(),
						para_source,
					});
				},
			}

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(0)]
		pub fn receive_auth_message(
			origin: OriginFor<T>,
			credential: BoundedVec<u8, T::MaxCredentialLength>,
			signature: [u8; 64],
		) -> DispatchResult {
			// will be executed in the local parachain (A)
			let para_b = ensure_sibling_para(<T as Config>::RuntimeOrigin::from(origin))?;
			let para_a = T::SelfParaId::get();

			Self::deposit_event(Event::ParaAuthCredentialIssued {
				credential,
				signature,
				para_source: para_a,
				para_remote: para_b,
			});
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(0)]
		pub fn request_local_auth(origin: OriginFor<T>, unique_id: [u8; 16]) -> DispatchResult {
			// will be executed in the local parachain (A)
			let sender = ensure_signed(origin)?;

			// 保证该用户存在
			if !OwnerOfCollectibles::<T>::contains_key(&sender) {
				Self::deposit_event(Event::LocalAuthFailed { account: sender.clone() });
				return Ok(());
			}

			// 保证该用户拥有该设备
			match OwnerOfCollectibles::<T>::get(&sender).iter().find(|&&x| x == unique_id) {
				Some(dev) => {
					Self::deposit_event(Event::LocalAuthSuccess {
						account: sender,
						unique_id: dev.clone(),
					});
				},
				None => {
					Self::deposit_event(Event::LocalAuthFailed { account: sender });
				},
			}

			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(0)]
		pub fn update_ca_pubkey(origin: OriginFor<T>, pubkey: [u8; 32]) -> DispatchResult {
			// will be executed in the local parachain (A)
			ensure_root(origin)?;
			if CAPublicKey::<T>::get() != pubkey {
				CAPublicKey::<T>::put(pubkey);
				Self::deposit_event(Event::CAPublicKeyUpdated { public_key: pubkey });
			}
			Ok(())
		}

		#[pallet::call_index(8)]
		#[pallet::weight(0)]
		pub fn issue_local_credential(
			origin: OriginFor<T>,
			credential: BoundedVec<u8, T::MaxCredentialLength>,
			signature: [u8; 64],
		) -> DispatchResult {
			ensure_root(origin)?;
			Self::deposit_event(Event::LocalAuthCredentialIssued { credential, signature });
			Ok(())
		}

		#[pallet::call_index(9)]
		#[pallet::weight(0)]
		pub fn send_credential(
			origin: OriginFor<T>,
			credential: BoundedVec<u8, T::MaxCredentialLength>,
			signature: [u8; 64],
			para: ParaId,
		) -> DispatchResult {
			// 发送B链的凭证到A链
			ensure_root(origin)?;

			match T::XcmSender::send_xcm(
				(1, Junction::Parachain(para.into())),
				Xcm(vec![Transact {
					origin_type: OriginKind::Native,
					require_weight_at_most: 1_000,
					call: <T as Config>::RuntimeCall::from(Call::<T>::receive_auth_message {
						credential,
						signature,
					})
					.encode()
					.into(),
				}]),
			) {
				Ok(()) => {
					Self::deposit_event(Event::TestMessage { message: 33 });
				},
				Err(_e) => {
					Self::deposit_event(Event::TestMessage { message: 44 });
				},
			};

			Ok(())
		}
	}
}
