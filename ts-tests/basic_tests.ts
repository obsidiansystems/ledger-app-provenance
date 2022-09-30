import { expect } from 'chai';
import { describe, it } from 'mocha';
import SpeculosTransport from '@ledgerhq/hw-transport-node-speculos';
import Axios from 'axios';
import Transport from "./common";
import { Common } from "hw-app-obsidian-common";
import * as blake2b from "blake2b";
import { instantiate, Nacl } from "js-nacl";

let ignoredScreens = [ "W e l c o m e", "Cancel", "Working...", "Exit", "Rust App 0.0.1"]

let setAcceptAutomationRules = async function() {
    await Axios.post("http://localhost:5000/automation", {
      version: 1,
      rules: [
        ... ignoredScreens.map(txt => { return { "text": txt, "actions": [] } }),
        { "y": 16, "actions": [] },
        { "text": "Confirm", "actions": [ [ "button", 1, true ], [ "button", 2, true ], [ "button", 2, false ], [ "button", 1, false ] ]},
        { "actions": [ [ "button", 2, true ], [ "button", 2, false ] ]}
      ]
    });
}

let processPrompts = function(prompts: [any]) {
  let i = prompts.filter((a : any) => !ignoredScreens.includes(a["text"])).values();
  let {done, value} = i.next();
  let header = "";
  let prompt = "";
  let rv = [];
  while(!done) {
    if(value["y"] == 1) {
      if(value["text"] != header) {
        if(header || prompt) rv.push({ header, prompt });
        header = value["text"];
        prompt = "";
      }
    } else if(value["y"] == 16) {
      prompt += value["text"];
    } else {
      if(header || prompt) rv.push({ header, prompt });
      rv.push(value);
      header = "";
      prompt = "";
    }
    ({done, value} = i.next());
  }
  return rv;
}

let sendCommandAndAccept = async function(command : any, prompts : any) {
    await setAcceptAutomationRules();
    await Axios.delete("http://localhost:5000/events");

    let transport = await Transport.open("http://localhost:5000/apdu");
    let client = new Common(transport, "rust-app");
    client.sendChunks = client.sendWithBlocks; // Use Block protocol
    let err = null;

    try { await command(client); } catch(e) {
      err = e;
    }
    if(err) throw(err);

    expect(processPrompts((await Axios.get("http://localhost:5000/events")).data["events"] as [any])).to.deep.equal(prompts);
}

describe('basic tests', () => {

  afterEach( async function() {
    await Axios.post("http://localhost:5000/automation", {version: 1, rules: []});
    await Axios.delete("http://localhost:5000/events");
  });

  it('provides a public key', async () => {

    await sendCommandAndAccept(async (client : Common) => {
      let rv = await client.getPublicKey("0");
      console.log(rv);
      //expect(rv.publicKey).to.equal("8118ad392b9276e348c1473649a3bbb7ec2b39380e40898d25b55e9e6ee94ca3");
      return;
    }, [
      { "header": "Provide Public Key", "prompt": "For Address     7f916b907886913c6dd7ab62681fc52140afbc84" },
      {
        "text": "Confirm",
        "x": 43,
        "y": 11,
      },
      {
        "text": "Provenance 0.0.1",
        "x": 20,
        "y": 11,
      }
    ]);
  });
});

let nacl : Nacl =null;

instantiate(n => { nacl=n; });

function testTransaction(path: string, txn: string, prompts: any[]) {
     return async () => {
       let sig = await sendCommandAndAccept(
         async (client : Common) => {

           let pubkey = (await client.getPublicKey(path)).publicKey;

           // We don't want the prompts from getPublicKey in our result
           await Axios.delete("http://localhost:5000/events");

           let sig = await client.signTransaction(path, Buffer.from(txn, "hex").toString("hex"));
           expect(sig.signature.length).to.equal(128);
	   // Skip verifying the signature
	   /*
           let hash = blake2b(32).update(Buffer.from(txn, "utf-8")).digest();
           let pass = nacl.crypto_sign_verify_detached(Buffer.from(sig.signature, 'hex'), hash, Buffer.from(pubkey, 'hex'));
           expect(pass).to.equal(true);
	  */
         }, prompts);
     }
}

describe("Protobufs tests", function() {
	this.timeout(30000);
	it("Can sign a send transaction", 
	   testTransaction("0/0",
             "0a90010a8b010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e64126b0a29747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b70386632667778122974703176786c63787032766a6e796a7577366d716e39643863713632636575366c6c6c7075736879361a130a056e68617368120a313630303030303030301200126d0a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a020801184e12190a130a056e68617368120a3137303238343532313010eefa041a0d70696f2d746573746e65742d3120ae59",[])
	  )
	it.skip("Can sign a delegate transaction", 
	   testTransaction("0/0",
             "0a9c010a99010a232f636f736d6f732e7374616b696e672e763162657461312e4d736744656c656761746512720a29747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781230747076616c6f706572317467713663707536686d7372766b76647538326a39397473787877377171616a6e38343366651a130a056e68617368120a32303030303030303030126d0a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a020801185212190a130a056e68617368120a3630393835363232323510fda6091a406d24f94f67322bdc8b5ab6b418a12ed872e8feed02411570ff62946130e51e4a62fed9ca3d8b3abaa0c0197f314ecf2b845d200ca3c584439f35478ca1dcc1bd",[])
	  )
	it("Can sign a send and delegate transaction", 
	   testTransaction("0/0",
             "0a9b020a89010a1c2f636f736d6f732e62616e6b2e763162657461312e4d736753656e6412690a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a110a056e68617368120831303030303030300a8c010a232f636f736d6f732e7374616b696e672e763162657461312e4d736744656c656761746512650a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a0d0a056e68617368120431303030124a12480a160a056e68617368120d3133373336393937363337303010d290ec011a29747031303530776b7a743764723734306a76703578703936766a71616d78356b70396a76706a7663751a0d70696f2d746573746e65742d3120e37c",[])
	  )
	it("Can sign a deposit transaction", 
	   testTransaction("0/0",
             "0a660a640a1e2f636f736d6f732e676f762e763162657461312e4d73674465706f7369741242084b1229747031673575676665676b6c35676d6e3034396e35613968676a6e3367656430656b703866326677781a130a056e68617368120a3530303030303030303012560a500a460a1f2f636f736d6f732e63727970746f2e736563703235366b312e5075624b657912230a2102da92ecc44eef3299e00cdf8f4768d5b606bf8242ff5277e6f07aadd935257a3712040a0208011852120210001a00",[])
	  )
})

// describe("Signing tests", function() {
//   before( async function() {
//     while(!nacl) await new Promise(r => setTimeout(r, 100));
//   })

//   it("can sign a transaction",
//      testTransaction(
//        "0",
//        JSON.stringify({"testapp":true}),
//        [
//          {
//            "header": "Transaction hash",
//            "prompt": "a5dQl_ZMC3Onv0ldlZ9C-Nl75FXraTHpoipEGTdNzrQ",
//          },
//          {
//            "header": "Sign for Address",
//            "prompt": "7f916b907886913c6dd7ab62681fc52140afbc84"
//          },
//          {
//            "text": "Sign Transaction?",
//            "x": 19,
//            "y": 11
//          },
//          {
//            "text": "Confirm",
//            "x": 43,
//            "y": 11,
//          }
//        ]
//      ));
// });
