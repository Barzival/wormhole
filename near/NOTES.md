
My impression is:

   https://docs.near.org/docs/tutorials/near-indexer

   https://thewiki.near.page/events-api   

==
kubectl exec -it near-0 -c near-node -- /bin/bash

My NEAR notes so far...

If needed, install `Rust`:

  curl https://sh.rustup.rs -sSf | sh

You need at least version 1.56 or later

  rustup default 1.56
  rustup update
  rustup target add wasm32-unknown-unknown

If needed, install `near-cli`:

   npm install near-cli -g

To install the npm dependencies of this test program

   npm install

for the near sdk, we are dependent on 4.0.0 or later  (where the ecrecover API is)

  https://docs.rs/near-sdk/4.0.0/near_sdk/index.html
  near-sdk = { version = "4.0.0", features = ["unstable"] }

  This has been stuck into Cargo.toml

to bring up the sandbox, start a tmux window and run

  rm -rf _sandbox
  mkdir -p _sandbox
  near-sandbox --home _sandbox init
  near-sandbox --home _sandbox run

https://docs.near.org/docs/develop/contracts/sandbox

First thing, lets put this in a docker in Tilt..

vaa_verify?

near-sdk-rs/near-sdk/src/environment/env.rs: (still unstable)

    /// Recovers an ECDSA signer address from a 32-byte message `hash` and a corresponding `signature`
    /// along with `v` recovery byte.
    ///
    /// Takes in an additional flag to check for malleability of the signature
    /// which is generally only ideal for transactions.
    ///
    /// Returns 64 bytes representing the public key if the recovery was successful.
    #[cfg(feature = "unstable")]
    pub fn ecrecover(
        hash: &[u8],
        signature: &[u8],
        v: u8,
        malleability_flag: bool,
    ) -> Option<[u8; 64]> {
        unsafe {
            let return_code = sys::ecrecover(
                hash.len() as _,
                hash.as_ptr() as _,
                signature.len() as _,
                signature.as_ptr() as _,
                v as u64,
                malleability_flag as u64,
                ATOMIC_OP_REGISTER,
            );
            if return_code == 0 {
                None
            } else {
                Some(read_register_fixed_64(ATOMIC_OP_REGISTER))
            }
        }
    }

you can look for test_ecrecover()    in the same file...

When building the sandbox, it is on port 3030 and we will need access to the validator_key.json...

curl http://localhost:3031/validator_key.json

function getConfig(env) {
  switch (env) {
    case "sandbox":
    case "local":
      return {
        networkId: "sandbox",
        nodeUrl: "http://localhost:3030",
        masterAccount: "test.near",
        contractAccount: "wormhole.test.near",
        keyPath: "./_sandbox/validator_key.json",
      };
  }
}

