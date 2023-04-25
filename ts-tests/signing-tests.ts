import { VERSION, sendCommandAndAccept, BASE_URL, sendCommandExpectFail, toggleBlindSigningSettings } from "./common";
import { expect } from 'chai';
import { describe, it } from 'mocha';
import Axios from 'axios';
import Provenance from "hw-app-hash";
import { ecdsaVerify } from 'secp256k1';
import { createHash } from 'crypto';

function testTransaction(path: string, txn0: string, prompts: any[]) {
  return async () => {
    await sendCommandAndAccept(async (client : Provenance) => {
      const txn = Buffer.from(txn0, "hex");
      const { publicKey } = await client.getPublicKey(path);
      // We don't want the prompts from getPublicKey in our result
      await Axios.delete(BASE_URL + "/events");

      const sig = await client.signTransaction(path, txn);
      expect(sig.signature.length).to.equal(64);
      const hash = createHash('sha256').update(txn).digest();
      const pass = ecdsaVerify(sig.signature, hash, publicKey);
      expect(pass).to.equal(true);
    }, prompts);
  }
}

describe("Protobufs tests", function() {
  this.timeout(30000);
  it("Can sign a send transaction",
    testTransaction("44'/505'/0'",
      "0a90010a8b010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e64126b0a29747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b70386632667778122974703176786c63787032766a6e796a7577366d716e39643863713632636575366c6c6c7075736879361a130a056e68617368120a313630303030303030301200126d0a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a020801184e12190a130a056e68617368120a3137303238343532313010eefa041a0d70696f2d746573746e65742d3120ae59",
      [
        {
          "header": "Transfer",
          "prompt": "HASH",
        },
        {
          "header": "From",
          "prompt": "tp1g5ugfegkl5gmn049n5a9hgjn3ged0ekp8f2fwx",
          "paginate": true,
        },
        {
          "header": "To",
          "prompt": "tp1vxlcxp2vjnyjuw6mqn9d8cq62ceu6lllpushy6",
          "paginate": true,
        },
        {
          "header": "Amount",
          "prompt": "1600000000 nhash",
        },
        {
          "header": "Chain ID",
          "prompt": "pio-testnet-1",
        },
        {
          "header": "With PKH",
          "prompt": "pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11
        },
      ])
  );
  it.skip("Can sign a delegate transaction",
    testTransaction("44'/505'/0'",
      "0a9c010a99010a232f636f736d6f732e7374616b696e672e763162657461312e4d736744656c656761746512720a29747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781230747076616c6f706572317467713663707536686d7372766b76647538326a39397473787877377171616a6e38343366651a130a056e68617368120a32303030303030303030126d0a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a020801185212190a130a056e68617368120a3630393835363232323510fda6091a406d24f94f67322bdc8b5ab6b418a12ed872e8feed02411570ff62946130e51e4a62fed9ca3d8b3abaa0c0197f314ecf2b845d200ca3c584439f35478ca1dcc1bd",
      [])
  );
  it("Can sign a send and delegate transaction",
    testTransaction("44'/505'/0'",
      "0a9b020a89010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412690a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a110a056e68617368120831303030303030300a8c010a232f636f736d6f732e7374616b696e672e763162657461312e4d736744656c656761746512650a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a0d0a056e68617368120431303030124a12480a160a056e68617368120d3133373336393937363337303010d290ec011a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751a0d70696f2d746573746e65742d3120e37c",
      [
        {
          "header": "Transfer",
          "prompt": "HASH",
        },
        {
          "header": "From",
          "prompt": "tp1050wkzt7dr740jvp5xp96vjqamx5kp9jvpjvcu",
          "paginate": true,
        },
        {
          "header": "To",
          "prompt": "tp1g5ugfegkl5gmn049n5a9hgjn3ged0ekp8f2fwx",
          "paginate": true,
        },
        {
          "header": "Amount",
          "prompt": "10000000 nhash",
        },
        {
          "header": "Delegate",
          "prompt": "",
        },
        {
          "header": "Delegator Address",
          "prompt": "tp1050wkzt7dr740jvp5xp96vjqamx5kp9jvpjvcu",
        },
        {
          "header": "Validator Address",
          "prompt": "tp1g5ugfegkl5gmn049n5a9hgjn3ged0ekp8f2fwx",
        },
        {
          "header": "Amount",
          "prompt": "1000 nhash",
        },
        {
          "header": "Chain ID",
          "prompt": "pio-testnet-1",
        },
        {
          "header": "With PKH",
          "prompt": "pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11,
        },
      ])
  );
  it("Can sign a deposit transaction",
    testTransaction("44'/505'/0'",
      "0a660a640a1e2f636f736d6f732e676f762e763162657461312e4d73674465706f7369741242084b1229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a130a056e68617368120a3530303030303030303012560a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a0208011852120210001a00",
      [
        {
          "header": "Proposal ID",
          "prompt": "75",
        },
        {
          "header": "Depositor Address",
          "prompt": "tp1g5ugfegkl5gmn049n5a9hgjn3ged0ekp8f2fwx",
        },
        {
          "header": "Amount",
          "prompt": "5000000000 nhash",
        },
        {
          "header": "Chain ID",
          "prompt": "",
        },
        {
          "header": "With PKH",
          "prompt": "pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11,
        },
      ])
  );
})
