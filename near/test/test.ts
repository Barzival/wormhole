// npx pretty-quick test.ts

const sha256 = require("js-sha256");
const nearAPI = require("near-api-js");
const BN = require("bn.js");
const fs = require("fs").promises;
const assert = require("assert").strict;
const fetch = require("node-fetch");
const elliptic = require("elliptic");
const web3Utils = require("web3-utils");
import { zeroPad } from "@ethersproject/bytes";
import { NodeHttpTransport } from "@improbable-eng/grpc-web-node-http-transport";

import { TestLib } from "./testlib";

import {
  ChainId,
  CHAIN_ID_ALGORAND,
  CHAIN_ID_NEAR,
} from "@certusone/wormhole-sdk/lib/cjs/utils";

import { _parseVAAAlgorand } from "@certusone/wormhole-sdk/lib/cjs/algorand";

import { getSignedVAAWithRetry } from "@certusone/wormhole-sdk";

function getConfig(env: any) {
  switch (env) {
    case "sandbox":
    case "local":
      return {
        networkId: "sandbox",
        nodeUrl: "http://localhost:3030",
        masterAccount: "test.near",
        wormholeAccount:
          Math.floor(Math.random() * 10000).toString() + "wormhole.test.near",
        tokenAccount:
          Math.floor(Math.random() * 10000).toString() + "token.test.near",
        testAccount:
          Math.floor(Math.random() * 10000).toString() + "test.test.near",
        userAccount:
          Math.floor(Math.random() * 10000).toString() + "user.test.near",
      };
  }
}

const wormholeMethods = {
  viewMethods: [],
  changeMethods: ["boot_wormhole", "submit_vaa"],
};

const tokenMethods = {
  viewMethods: [],
  changeMethods: [
    "boot_portal",
    "submit_vaa",
    "submit_vaa_callback",
    "attest_near",
    "attest_token",
    "send_transfer_near",
    "send_transfer_token",
    "account_hash",
  ],
};

const testMethods = {
  viewMethods: [],
  changeMethods: ["deploy_ft"],
};

const ftMethods = {
  viewMethods: [],
  changeMethods: ["ft_transfer_call"],
};

let config: any;
let masterAccount: any;
let _tokenAccount: any;
let _wormholeAccount: any;
let _testAccount: any;
let masterKey: any;
let masterPubKey: any;
let keyStore: any;
let near: any;

let userAccount: any;
let userKey: any;
let userPubKey: any;

async function initNear() {
  config = getConfig(process.env.NEAR_ENV || "sandbox");

  // Retrieve the validator key directly in the Tilt environment
  const response = await fetch("http://localhost:3031/validator_key.json");

  const keyFile = await response.json();

  console.log(keyFile);

  masterKey = nearAPI.utils.KeyPair.fromString(
    keyFile.secret_key || keyFile.private_key
  );
  masterPubKey = masterKey.getPublicKey();

  userKey = nearAPI.utils.KeyPair.fromRandom("ed25519");
  console.log(userKey);

  keyStore = new nearAPI.keyStores.InMemoryKeyStore();

  keyStore.setKey(config.networkId, config.masterAccount, masterKey);
  keyStore.setKey(config.networkId, config.userAccount, userKey);

  near = await nearAPI.connect({
    deps: {
      keyStore,
    },
    networkId: config.networkId,
    nodeUrl: config.nodeUrl,
  });
  masterAccount = new nearAPI.Account(near.connection, config.masterAccount);

  console.log(
    "Finish init NEAR masterAccount: " +
      JSON.stringify(await masterAccount.getAccountBalance())
  );

  let resp = await masterAccount.createAccount(
    config.userAccount,
    userKey.getPublicKey(),
    new BN(10).pow(new BN(25))
  );

  console.log(resp);

  userAccount = new nearAPI.Account(near.connection, config.userAccount);

  console.log(
    "Finish init NEAR userAccount: " +
      JSON.stringify(await userAccount.getAccountBalance())
  );

  //  console.log(await userAccount.sendMoney(config.masterAccount, nearAPI.utils.format.parseNearAmount("1.5")));;
  //  console.log("Sent some money: " + JSON.stringify(await userAccount.getAccountBalance()));
}

async function createContractUser(
  accountPrefix: any,
  contractAccountId: any,
  methods: any
) {
  let accountId =
    Math.floor(Math.random() * 10000).toString() +
    accountPrefix +
    "." +
    config.masterAccount;

  console.log(accountId);

  let randomKey = nearAPI.utils.KeyPair.fromRandom("ed25519");

  let resp = await masterAccount.createAccount(
    accountId,
    randomKey.getPublicKey(),
    new BN(10).pow(new BN(28))
  );
  console.log("accountId: " + JSON.stringify(resp));

  keyStore.setKey(config.networkId, accountId, randomKey);
  const account = new nearAPI.Account(near.connection, accountId);
  const accountUseContract = new nearAPI.Contract(
    account,
    contractAccountId,
    methods
  );
  return accountUseContract;
}

async function initTest() {
  const wormholeContract = await fs.readFile(
    "./contracts/wormhole/target/wasm32-unknown-unknown/release/wormhole.wasm"
  );
  const tokenContract = await fs.readFile(
    "./contracts/portal/target/wasm32-unknown-unknown/release/portal.wasm"
  );
  const testContract = await fs.readFile(
    "./contracts/mock-bridge-integration/target/wasm32-unknown-unknown/release/mock_bridge_integration.wasm"
  );

  let randomKey = nearAPI.utils.KeyPair.fromRandom("ed25519");
  keyStore.setKey(config.networkId, config.wormholeAccount, randomKey);

  _wormholeAccount = await masterAccount.createAndDeployContract(
    config.wormholeAccount,
    randomKey.getPublicKey(),
    wormholeContract,
    new BN(10).pow(new BN(27))
  );

  randomKey = nearAPI.utils.KeyPair.fromRandom("ed25519");
  keyStore.setKey(config.networkId, config.tokenAccount, randomKey);

  _tokenAccount = await masterAccount.createAndDeployContract(
    config.tokenAccount,
    randomKey.getPublicKey(),
    tokenContract,
    new BN(10).pow(new BN(27))
  );

  console.log("tokenAccount: " + config.tokenAccount);

  _testAccount = await masterAccount.createAndDeployContract(
    config.testAccount,
    randomKey.getPublicKey(),
    testContract,
    new BN(10).pow(new BN(27))
  );

  const wormholeUseContract = await createContractUser(
    "wormhole_user",
    config.wormholeAccount,
    wormholeMethods
  );

  const tokenUseContract = await createContractUser(
    "tokenbridge_user",
    config.tokenAccount,
    tokenMethods
  );

  const testUseContract = await createContractUser(
    "test_user",
    config.testAccount,
    testMethods
  );

  //
  //  console.log(userUseContract.account.accountId);

  console.log("Finish deploy contracts and create test accounts");
  return {
    wormholeUseContract,
    tokenUseContract,
    testUseContract,
  };
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function test() {
  let fastTest = true;
  let ts = new TestLib();

  await initNear();
  const { wormholeUseContract, tokenUseContract, testUseContract } =
    await initTest();

  console.log("Booting guardian set with index 0");
  await wormholeUseContract.boot_wormhole({
    args: { gset: 0, addresses: ts.guardianKeys },
  });
  console.log("Completed without an error... odd.. I am not sucking yet");

  console.log("Booting up the token bridge");
  await tokenUseContract.boot_portal({
    args: { core: config.wormholeAccount },
  });
  console.log("token bridge booted");

  let seq = 1;

  console.log("lets upgrade the governance set to 1");
  let vaa = ts.genGuardianSetUpgrade(
    ts.guardianPrivKeys,
    0,
    1,
    1,
    seq,
    ts.guardianKeys
  );

  console.log("sending it to the core contract");
  await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });

  seq = seq + 1;

  if (!fastTest) {
    console.log("Its parsed... lets do it again!!");
    try {
      await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. nice.. we dont suck");
    }

    console.log(
      "Lets try to send a governence message (SetFee) with the wrong index"
    );
    vaa = ts.genGSetFee(ts.guardianPrivKeys, 0, 1, seq, CHAIN_ID_NEAR, 5);
    try {
      await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log(
        "Exception thrown.. nice..  this was with the wrong governance set"
      );
    }

    console.log(
      "Lets try to send a governence message (SetFee) with the correct index but the wrong chain"
    );
    vaa = ts.genGSetFee(ts.guardianPrivKeys, 1, 1, seq, CHAIN_ID_ALGORAND, 5);
    try {
      await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }

    console.log(
      "Lets try to send a governence message (SetFee) with the correct index but for all chains"
    );
    vaa = ts.genGSetFee(ts.guardianPrivKeys, 1, 1, seq, 0, 5);
    try {
      await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }

    console.log(
      "Lets try to send a governence message (SetFee)  with the correct index and the correct chain"
    );

    vaa = ts.genGSetFee(ts.guardianPrivKeys, 1, 1, seq, CHAIN_ID_NEAR, 5);
    await wormholeUseContract.submit_vaa({ args: { vaa: vaa } });
    console.log("boo yaah! this was supposed to pass and it did");

    seq = seq + 1;

    console.log("lets try to call the vaa_vallback directly");
    try {
      await tokenUseContract.submit_vaa_callback({ args: {} });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }

    try {
      vaa = ts.genRegisterChain(ts.guardianPrivKeys, 0, 1, seq, 1);
      console.log(
        "Now lets call submit_vaa with a valid vaa (register the solana chain) on the token bridge.. with the wrong governance set"
      );
      await tokenUseContract.submit_vaa({
        args: { vaa: vaa },
        gas: 300000000000000,
      });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }
  }

  vaa = ts.genRegisterChain(ts.guardianPrivKeys, 1, 1, seq, 1);
  console.log(
    "Now lets call submit_vaa with a valid vaa (register the solana chain) on the token bridge.. with the correct governance set"
  );
  await tokenUseContract.submit_vaa({
    args: { vaa: vaa },
    gas: 300000000000000,
  });

  seq = seq + 1;

  if (!fastTest) {
    try {
      vaa = ts.genRegisterChain(ts.guardianPrivKeys, 1, 1, seq, 1);
      console.log(
        "Now lets call submit_vaa with a valid vaa (register the solana chain) again.. again... this should fail"
      );
      await tokenUseContract.submit_vaa({
        args: { vaa: vaa },
        gas: 300000000000000,
      });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }
  }

  let usdcEmitter = "4523c3F29447d1f32AEa95BEBD00383c4640F1b4".toLowerCase();

  if (!fastTest) {
    try {
      vaa =
        ts.genAssetMeta(
          ts.guardianPrivKeys,
          1,
          1,
          seq,
          usdcEmitter,
          1,
          8,
          "USDC",
          "CircleCoin"
        ) + "00";
      console.log(
        "Now the fun stuff... lets create some USDC... but pass a hacked vaa"
      );
      await tokenUseContract.submit_vaa({
        args: { vaa: vaa },
        gas: 300000000000000,
      });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }
  }
  vaa = ts.genAssetMeta(
    ts.guardianPrivKeys,
    1,
    1,
    seq,
    usdcEmitter,
    1,
    8,
    "USDC2",
    "CircleCoin2"
  );
  console.log("Now the fun stuff... lets create some USDC");
  await tokenUseContract.submit_vaa({
    args: { vaa: vaa },
    gas: 300000000000000,
  });
  console.log("Try again (since this is an attest)");
  await tokenUseContract.submit_vaa({
    args: { vaa: vaa },
    gas: 300000000000000,
    amount: "33000000000000000000000000",
  });

  let tname = usdcEmitter + "0001." + config.tokenAccount;

  console.log(tname);

  seq = seq + 1;

  if (!fastTest) {
    console.log("Lets attest the same thing again");
    vaa = ts.genAssetMeta(
      ts.guardianPrivKeys,
      1,
      1,
      seq,
      usdcEmitter,
      1,
      8,
      "USDC",
      "CircleCoin"
    );
    await tokenUseContract.submit_vaa({
      args: { vaa: vaa },
      gas: 300000000000000,
    });
    await tokenUseContract.submit_vaa({
      args: { vaa: vaa },
      gas: 300000000000000,
      amount: "20000000000000000000000000",
    });

    seq = seq + 1;

    try {
      console.log("Lets make it fail");
      vaa = ts.genAssetMeta(
        ts.guardianPrivKeys,
        1,
        1,
        seq,
        usdcEmitter,
        1,
        8,
        "USDC20",
        "OnceUponATimeFarAway"
      );
      await tokenUseContract.submit_vaa({
        args: { vaa: vaa },
        gas: 300000000000000,
      });
      await tokenUseContract.submit_vaa({
        args: { vaa: vaa },
        gas: 300000000000000,
      });
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }

    seq = seq + 1;
  }

  console.log(
    "Now, for something useful? send some USDC to our test account: " +
      tokenUseContract.account.accountId
  );

  vaa = ts.genTransfer(
    ts.guardianPrivKeys,
    1,
    1,
    seq,
    100000,
    usdcEmitter,
    1,
    Buffer.from(tokenUseContract.account.accountId).toString("hex"),
    CHAIN_ID_NEAR,
    0
  );
  console.log(vaa);
  console.log(_parseVAAAlgorand(ts.hexStringToUint8Array(vaa)));
  await tokenUseContract.submit_vaa({
    args: { vaa: vaa },
    gas: 300000000000000,
  });
  console.log("well?  did it work?!");

  console.log("npm i -g near-cli");
  console.log(
    "near --nodeUrl http://localhost:3030 view " +
      tname +
      ' ft_balance_of \'{"account_id": "' +
      tokenUseContract.account.accountId +
      "\"}'"
  );

  seq = seq + 1;

  if (!fastTest) {
    try {
      console.log(
        "attesting near.. but not attaching any cash to cover this as the first time this emitter is being seen"
      );
      let sequence = await tokenUseContract.attest_near({
        args: {},
        gas: 100000000000000,
      });
      console.log(sequence);
      console.log("This should have thrown a exception..");
      process.exit(1);
    } catch {
      console.log("Exception thrown.. that is correct...   ");
    }
  }
  let ah = await tokenUseContract.account_hash({ args: {} });
  console.log(ah);
  let emitter = sha256.sha256.hex(ah[0]);
  if (ah[0] != config.tokenAccount) {
    console.log("The token account does not match what I think it should be");
    process.exit(1);
  }
  if (ah[1] != emitter) {
    console.log(
      "The sha256 hash of the token account does not match what I think it should be"
    );
    process.exit(1);
  }
  console.log("emitter: " + emitter);

  if (!fastTest) {
    {
      console.log("attesting near but paying for it ");
      let sequence = await tokenUseContract.attest_near({
        args: {},
        gas: 100000000000000,
        amount: "790000000000000000000",
      });
      console.log(sequence);

      const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
        ["http://localhost:7071"],
        CHAIN_ID_NEAR,
        emitter,
        sequence,
        {
          transport: NodeHttpTransport(),
        }
      );

      console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
    }

    {
      console.log("attesting wormhole token: " + tname);
      let sequence = await tokenUseContract.attest_token({
        args: { token: tname },
        gas: 100000000000000,
      });
      console.log(sequence);

      const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
        ["http://localhost:7071"],
        CHAIN_ID_NEAR,
        emitter,
        sequence,
        {
          transport: NodeHttpTransport(),
        }
      );

      console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
    }

    console.log("deploying a random token");
    let randoToken = await testUseContract.deploy_ft({
      args: {},
      gas: 300000000000000,
    });

    {
      console.log("attesting non wormhole token: " + randoToken);
      let sequence = await tokenUseContract.attest_token({
        args: { token: randoToken },
        gas: 100000000000000,
      });
      console.log(sequence);

      const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
        ["http://localhost:7071"],
        CHAIN_ID_NEAR,
        emitter,
        sequence,
        {
          transport: NodeHttpTransport(),
        }
      );

      console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
      console.log(_parseVAAAlgorand(signedVAA));
    }

    {
      console.log("sending some NEAR off to a HODLr in the cloud");
      let sequence = await tokenUseContract.send_transfer_near({
        args: { receiver: "0011223344", chain: 1, fee: 0, payload: "" },
        amount: "7900000000000001234",
        gas: 100000000000000,
      });
      console.log(sequence);

      const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
        ["http://localhost:7071"],
        CHAIN_ID_NEAR,
        emitter,
        sequence,
        {
          transport: NodeHttpTransport(),
        }
      );

      console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
      console.log(_parseVAAAlgorand(signedVAA));
    }
  }

  //  {
  //    console.log("sending some RANDO off to a HODLr in the cloud");
  //    let sequence = await tokenUseContract.send_transfer_token({
  //      args: { token: randoToken, receiver: "0011223344", chain: 1, fee: 0, payload: "" },
  //      amount: "7900000000000001234",
  //      gas: 100000000000000,
  //    });
  //    console.log(sequence);
  //
  //    const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
  //      ["http://localhost:7071"],
  //      CHAIN_ID_NEAR,
  //      emitter,
  //      sequence,
  //      {
  //        transport: NodeHttpTransport(),
  //      }
  //    );
  //
  //    console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
  //    console.log(_parseVAAAlgorand(signedVAA));
  //  }

  {
    console.log("sending some USDC off to a HODLr in the cloud: " + tname);
    console.log(ah);
    let sequence = await tokenUseContract.send_transfer_token({
      args: {
        amount: 12345,
        token: tname,
        receiver: "0011223344",
        chain: 1,
        fee: 0,
        payload: "",
      },
      amount: "800000000000000000000",
      gas: 100000000000000,
    });
    console.log(sequence);

    const { vaaBytes: signedVAA } = await getSignedVAAWithRetry(
      ["http://localhost:7071"],
      CHAIN_ID_NEAR,
      emitter,
      sequence,
      {
        transport: NodeHttpTransport(),
      }
    );

    console.log("vaa: " + Buffer.from(signedVAA).toString("hex"));
    console.log(_parseVAAAlgorand(signedVAA));
  }

//  const tokenContract = new nearAPI.Contract(
//    testUseContract,
//    randoToken,
//    ftMethods
//  );

  console.log("test complete");
}

test();
