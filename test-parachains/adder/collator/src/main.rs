// Copyright 2018-2020 Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

//! Collator for polkadot

use std::collections::HashMap;
use std::sync::Arc;

use adder::{HeadData as AdderHead, BlockData as AdderBody};
use sp_core::Pair;
use codec::{Encode, Decode};
use primitives::{
	Hash,
	parachain::{HeadData, BlockData, Id as ParaId, LocalValidationData},
};
use collator::{
	InvalidHead, ParachainContext, Network, BuildParachainContext, load_spec, Configuration,
};
use parking_lot::Mutex;
use futures::future::{Ready, ok, err};

const GENESIS: AdderHead = AdderHead {
	number: 0,
	parent_hash: [0; 32],
	post_state: [
		1, 27, 77, 3, 221, 140, 1, 241, 4, 145, 67, 207, 156, 76, 129, 126, 75,
		22, 127, 29, 27, 131, 229, 198, 240, 241, 13, 137, 186, 30, 123, 206
	],
};

const GENESIS_BODY: AdderBody = AdderBody {
	state: 0,
	add: 0,
};

#[derive(Clone)]
struct AdderContext {
	db: Arc<Mutex<HashMap<AdderHead, AdderBody>>>,
	/// We store it here to make sure that our interfaces require the correct bounds.
	_network: Option<Arc<dyn Network>>,
}

/// The parachain context.
impl ParachainContext for AdderContext {
	type ProduceCandidate = Ready<Result<(BlockData, HeadData), InvalidHead>>;

	fn produce_candidate(
		&mut self,
		_relay_parent: Hash,
		local_validation: LocalValidationData,
	) -> Self::ProduceCandidate
	{
		let adder_head = match AdderHead::decode(&mut &local_validation.parent_head.0[..]) {
			Ok(adder_head) => adder_head,
			Err(_) => return err(InvalidHead)
		};

		let mut db = self.db.lock();

		let last_body = if adder_head == GENESIS {
			GENESIS_BODY
		} else {
			db.get(&adder_head)
				.expect("All past bodies stored since this is the only collator")
				.clone()
		};

		let next_body = AdderBody {
			state: last_body.state.overflowing_add(last_body.add).0,
			add: adder_head.number % 100,
		};

		let next_head = ::adder::execute(adder_head.hash(), adder_head, &next_body)
			.expect("good execution params; qed");

		let encoded_head = HeadData(next_head.encode());
		let encoded_body = BlockData(next_body.encode());

		println!("Created collation for #{}, post-state={}",
			next_head.number, next_body.state.overflowing_add(next_body.add).0);

		db.insert(next_head.clone(), next_body);
		ok((encoded_body, encoded_head))
	}
}

impl BuildParachainContext for AdderContext {
	type ParachainContext = Self;

	fn build<B, E, R, SP, Extrinsic>(
		self,
		_: Arc<collator::PolkadotClient<B, E, R>>,
		_: SP,
		network: impl Network + Clone + 'static,
	) -> Result<Self::ParachainContext, ()> {
		Ok(Self { _network: Some(Arc::new(network)), ..self })
	}
}

fn main() {
	let key = Arc::new(Pair::from_seed(&[1; 32]));
	let id: ParaId = 100.into();

	println!("Starting adder collator with genesis: ");

	{
		let encoded = GENESIS.encode();
		println!("Dec: {:?}", encoded);
		print!("Hex: 0x");
		for byte in encoded {
			print!("{:02x}", byte);
		}

		println!();
	}

	let context = AdderContext {
		db: Arc::new(Mutex::new(HashMap::new())),
		_network: None,
	};

	let mut config = Configuration::default();
	config.chain_spec = Some(load_spec("dev", false).unwrap());

	let res = collator::run_collator(
		context,
		id,
		key,
		config,
	);

	if let Err(e) = res {
		println!("{}", e);
	}
}
