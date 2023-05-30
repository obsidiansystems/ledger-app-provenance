import { sendCommandAndAccept, BASE_URL, } from "./common";
import { expect } from 'chai';
import { describe, it } from 'mocha';
import Axios from 'axios';
import Provenance from "hw-app-hash";

describe('public key tests', () => {

  afterEach( async function() {
    await Axios.post(BASE_URL + "/automation", {version: 1, rules: []});
    await Axios.delete(BASE_URL + "/events");
  });

  it('provides a public key', async () => {

    await sendCommandAndAccept(async (client : Provenance) => {
      let rv = await client.getPublicKey("44'/505'/0'");
      expect(new Buffer(rv.address).toString()).to.equal("pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd");
      expect(new Buffer(rv.publicKey).toString('hex')).to.equal("0368a7dc46a8c9e99872567b84cb6693b07f04ad25c9e8f8377654f4772d35cff1");
      return;
    }, []);
  });

  it('provides a public key 2', async () => {

    await sendCommandAndAccept(async (client : Provenance) => {
      let rv = await client.getPublicKey("44'/505'");
      expect(new Buffer(rv.address).toString()).to.equal("pb1hqrpuntc0yew7q7ts6h8hqvlccsqhhy3m62l7x");
      expect(new Buffer(rv.publicKey).toString('hex')).to.equal("03bd3617cd8eb3d36449f7a4f7df5bc89e24615d0bac4bc82b34fb56a2f377677e");
      return;
    }, []);
  });

  it('does address verification', async () => {

    await sendCommandAndAccept(async (client : Provenance) => {
      const rv = await client.verifyAddress("44'/505'/0'");
      expect(new Buffer(rv.address).toString()).to.equal("pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd");
      expect(new Buffer(rv.publicKey).toString('hex')).to.equal("0368a7dc46a8c9e99872567b84cb6693b07f04ad25c9e8f8377654f4772d35cff1");
      return;
    }, [
      {
        "header": "Provide Public Key",
        "prompt": "",
      },
      {
        "header": "Address",
        "prompt": "pb1lem544f29gucu09698cyz6z2y043j0wclrjgwd",
        "paginate": true,
      },
      {
        "text": "Confirm",
        "x": 43,
        "y": 11,
      },
    ]);
  });
});
