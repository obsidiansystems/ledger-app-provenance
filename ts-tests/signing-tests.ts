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
          "prompt": "1.6 hash",
        },
        {
          "header": "Fees",
          "prompt": "1.70284521 hash",
        },
        {
          "header": "Gas Limit",
          "prompt": "81262",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11
        },
      ])
  );
  it("Can sign a send transaction (2)",
     // https://explorer.provenance.io/tx/3877B06AD96A7AF8D7A944D1D7450EBA4836AF7491099E5E81B0F24E59BD9B5A/10597048
    testTransaction("44'/505'/0'",
      Buffer.from("CpIBCo8BChwvY29zbW9zLmJhbmsudjFiZXRhMS5Nc2dTZW5kEm8KKXBiMWtxdXZoOW1xa3puNnFrc2xjc3dtNjRhZWZjZ242OXc2cnE4bmY0EilwYjF6c2hlcnIzZWF0Nmd2cTlwdGczbTBuM2RqMzN4ZjJtd2V2azNjYRoXCgVuaGFzaBIOMTAwMDAwMDAwMDAwMDASbQpRCkYKHy9jb3Ntb3MuY3J5cHRvLnNlY3AyNTZrMS5QdWJLZXkSIwohAl6z93YkvBJAso9foCgXIRyOyXPo9Uwt2mDpQg/Lj6c9EgQKAggBGJwDEhgKEgoFbmhhc2gSCTE2Njc3NzAzNRD7qwU=", "base64").toString("hex"),

      [
        {
          "header": "Transfer",
          "prompt": "HASH",
        },
        {
          "header": "From",
          "prompt": "pb1kquvh9mqkzn6qkslcswm64aefcgn69w6rq8nf4",
          "paginate": true,
        },
        {
          "header": "To",
          "prompt": "pb1zsherr3eat6gvq9ptg3m0n3dj33xf2mwevk3ca",
          "paginate": true,
        },
        {
          "header": "Amount",
          "prompt": "10000.0 hash",
        },
        {
          "header": "Fees",
          "prompt": "0.166777035 hash",
        },
        {
          "header": "Gas Limit",
          "prompt": "87547",
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
          "prompt": "0.01 hash",
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
          "prompt": "0.000001 hash",
        },
        {
          "header": "Fees",
          "prompt": "1373.6997637 hash",
        },
        {
          "header": "Gas Limit",
          "prompt": "3868754",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11,
        },
      ])
  );
  it("Can sign a deposit transaction",
     // https://explorer.provenance.io/tx/BE8AF9CF1207EF595D09C64FDE6BCD4850ED837C0659379CE0FAB5417E4402B7/9501957
    testTransaction("44'/505'/0'",
      Buffer.from("CmUKYwoZL2Nvc21vcy5nb3YudjEuTXNnRGVwb3NpdBJGCC8SKXBiMXZqMHcwYXNnamRubGF0M2poMHAzYTRlNGphbXU4cDI2eTIwMDBrGhcKBW5oYXNoEg40OTk5MDAwMDAwMDAwMBJsClAKRgofL2Nvc21vcy5jcnlwdG8uc2VjcDI1NmsxLlB1YktleRIjCiECjRH5YYOCYVTDDV9cgZaE9tul9n87abNghgGfm2oKCFcSBAoCCAEYAhIYChIKBW5oYXNoEgkzODEwMDAwMDAQwJoM", "base64").toString("hex"),
      [
        {
          "header": "Proposal ID",
          "prompt": "47",
        },
        {
          "header": "Depositor Address",
          "prompt": "pb1vj0w0asgjdnlat3jh0p3a4e4jamu8p26y2000k",
        },
        {
          "header": "Amount",
          "prompt": "49990.0 hash",
        },
        {
          "header": "Fees",
          "prompt": "0.381 hash",
        },
        {
          "header": "Gas Limit",
          "prompt": "200000",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11,
        },
      ])
  );

  it("Can sign an undelegate transaction",
     // https://explorer.provenance.io/tx/AEDCCAC1DF43537D459DE6E28E62C6211366975ECBD2074C9D44488847E1A4AD/10575218
    testTransaction("44'/505'/0'",
      Buffer.from("Cp8BCpwBCiUvY29zbW9zLnN0YWtpbmcudjFiZXRhMS5Nc2dVbmRlbGVnYXRlEnMKKXBiMWNwN2x2dmRoNWs3eThqMDN6cGtrbmt1M2gwNmMzdzlxbmw5Y2wzEjBwYnZhbG9wZXIxY3A3bHZ2ZGg1azd5OGowM3pwa2tua3UzaDA2YzN3OXF1Z2ZnN3QaFAoFbmhhc2gSCzkzMTQ4NzA3NTY1Em0KUApGCh8vY29zbW9zLmNyeXB0by5zZWNwMjU2azEuUHViS2V5EiMKIQNGKvLHB9R8a1+lW7noW9sFAFMsHoaDmFK+32GQGpbqCBIECgIIARgWEhkKEwoFbmhhc2gSCjEwMTQ1MzIwMDAQgvse", "base64").toString("hex"),
      [
        {
          "header": "Undelegate",
          "prompt": "",
        },
        {
          "header": "Delegator Address",
          "prompt": "pb1cp7lvvdh5k7y8j03zpkknku3h06c3w9qnl9cl3",
        },
        {
          "header": "Validator Address",
          "prompt": "pbvaloper1cp7lvvdh5k7y8j03zpkknku3h06c3w9qugfg7t",
        },
        {
          "header": "Amount",
          "prompt": "93.148707565 hash",
        },
        {
          "header": "Fees",
          "prompt": "1.014532 hash",
        },
        {
          "header": "Gas Limit",
          "prompt": "507266",
        },
        {
          "text": "Confirm",
          "x": 43,
          "y": 11,
        },
      ])
  );
})
