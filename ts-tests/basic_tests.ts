import { expect } from 'chai';
import { describe, it } from 'mocha';
import SpeculosTransport from '@ledgerhq/hw-transport-node-speculos';
import Axios from 'axios';
import Transport from "./common";
import Provenance from "hw-app-hash";

let ignoredScreens = [ "W e l c o m e", "Cancel", "Working...", "Exit", "Provenance 0.0.1"]

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
    let kda = new Provenance(transport);
    
    //await new Promise(resolve => setTimeout(resolve, 100));
    
    let err = null;

    try { await command(kda); } catch(e) {
      err = e;
    }
    
    //await new Promise(resolve => setTimeout(resolve, 100));


    expect(processPrompts((await Axios.get("http://localhost:5000/events")).data["events"] as [any])).to.deep.equal(prompts);
    // expect(((await Axios.get("http://localhost:5000/events")).data["events"] as [any]).filter((a : any) => a["text"] != "W e l c o m e")).to.deep.equal(prompts);
    if(err) throw(err);
}

describe('basic tests', () => {
  afterEach( async function() {
    console.log("Clearing settings");
    await Axios.post("http://localhost:5000/automation", {version: 1, rules: []});
    await Axios.delete("http://localhost:5000/events");
  });

  it('provides a public key', async () => {

    await sendCommandAndAccept(async (hash_token : Provenance) => {
      console.log("Started pubkey get");
      let rv = await hash_token.getPublicKey("0");
      console.log("Reached Pubkey Got");
      expect(rv.publicKey).to.equal("026f760e57383e3b5900f7c23b78a424e74bebbe9b7b46316da7c0b4b9c2c9301c");
      return;
    }, [
      { "header": "Provide Public Key", "prompt": "For Address     ABF20C51EFFB2152DFE06C2F7B96138CABD69AD1" },
      {
        "text": "Confirm",
        "x": 43,
        "y": 11,
      },
    ]);
  });
  
  it('provides a public key', async () => {
  await sendCommandAndAccept(async (kda : Provenance) => {
      console.log("Started pubkey get");
      let rv = await kda.getPublicKey("0");
      console.log("Reached Pubkey Got, " + JSON.stringify(rv));
      expect(rv.publicKey).to.equal("026f760e57383e3b5900f7c23b78a424e74bebbe9b7b46316da7c0b4b9c2c9301c");
      return;
    },
    [
      { "header": "Provide Public Key", "prompt": "For Address     ABF20C51EFFB2152DFE06C2F7B96138CABD69AD1" },
      {
        "text": "Confirm",
        "x": 43,
        "y": 11,
      },
    ]);
  });
});

function testTransaction(path: string, txn: string, prompts: any[]) {
     return async () => {
       await sendCommandAndAccept(
         async (kda : Provenance) => {
           console.log("Started pubkey get");
           let rv = await kda.signTransaction(path, Buffer.from(txn, "utf-8").toString("hex"));
           expect(rv.signature.length).to.equal(128);
         }, prompts);
     }
}

// These tests have been extracted interacting with the testnet via the cli.

let exampleSend = {
  "chain_id": "testnet",
  "entropy": "-7780543831205109370",
  "fee": [
    {
      "amount": "10000",
      "denom": "uhash_token"
    }
  ],
  "memo": "Fourth transaction",
  "msgs": [
    {
      "type": "cosmos-sdk/Send",
      "value": {
        "amount": "1000000",
        "from_address": "db987ccfa2a71b2ec9a56c88c77a7cf66d01d8ba",
        "to_address": "db987ccfa2a71b2ec9a56c88c77a7cf66d01d8ba"
      }
    }
  ]
}

describe("Signing tests", function() {
  it("can sign a simple transfer",
     testTransaction(
       "0/0",
       JSON.stringify(exampleSend),
       [
         {
        "header": "Send",
        "prompt": "Transaction",
         },
         {
        "header": "Value",
        "prompt": "1000000",
         },
         {
        "header": "Transfer from",
        "prompt": "db987ccfa2a71b2ec9a56c88c77a7cf66d01d8ba",
         },
         {
        "header": "Transfer To",
        "prompt": "db987ccfa2a71b2ec9a56c88c77a7cf66d01d8ba",
         },
         {
        "header": "Sign Hash?",
        "prompt": "8596AE17444A83FADC3DD318BF0836574B84D742810972F5364DA73ED11EDC70",
         },
         {
        "header": "For Account",
        "prompt": "2E27FC80E710265D4CD47A4A44D3C1AE4F88DAAA"
         },
         {
           "text": "Confirm",
           "x": 43,
           "y": 11,
         }
       ]
     ));
});
