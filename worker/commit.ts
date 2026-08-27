// Server-signed devnet policy commitments for the demo.
//
// The product's claim is that a machine's exit policy is committed on-chain
// *before* any sale follows it. A visitor without a wallet could never see
// that happen. So the demo signs its own commitments with a throwaway devnet
// keypair (a Worker secret, funded with devnet SOL) and shows the visitor the
// real transaction. Mainnet commitments are always signed by the user's own
// wallet — this path exists only so the claim is visible without one.
//
// No web3.js: the transaction is built and signed by hand (WebCrypto Ed25519)
// to keep the bundle small and the dependency surface at zero. PDAs cannot be
// derived here (on-curve check), so they are precomputed in pda_table.json.

const SYSTEM_PROGRAM = "11111111111111111111111111111111";
const IX_COMMIT_POLICY = 0;

const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

export function b58decode(str: string): Uint8Array {
  let num = 0n;
  for (const ch of str) {
    const idx = B58.indexOf(ch);
    if (idx < 0) throw new Error("bad base58");
    num = num * 58n + BigInt(idx);
  }
  const bytes: number[] = [];
  while (num > 0n) {
    bytes.unshift(Number(num % 256n));
    num /= 256n;
  }
  for (const ch of str) {
    if (ch !== "1") break;
    bytes.unshift(0);
  }
  return Uint8Array.from(bytes);
}

export function b58encode(bytes: Uint8Array): string {
  let num = 0n;
  for (const b of bytes) num = num * 256n + BigInt(b);
  let out = "";
  while (num > 0n) {
    out = B58[Number(num % 58n)] + out;
    num /= 58n;
  }
  for (const b of bytes) {
    if (b !== 0) break;
    out = "1" + out;
  }
  return out;
}

/** Compact-u16 (shortvec) length prefix used throughout Solana messages. */
function shortvec(n: number): number[] {
  const out: number[] = [];
  let rem = n;
  for (;;) {
    if (rem < 0x80) {
      out.push(rem);
      return out;
    }
    out.push((rem & 0x7f) | 0x80);
    rem >>= 7;
  }
}

/** PKCS#8 wrapper so WebCrypto will import a raw 32-byte Ed25519 seed. */
function pkcs8(seed: Uint8Array): Uint8Array {
  const prefix = Uint8Array.from([
    0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70,
    0x04, 0x22, 0x04, 0x20,
  ]);
  const out = new Uint8Array(prefix.length + 32);
  out.set(prefix);
  out.set(seed.slice(0, 32), prefix.length);
  return out;
}

export interface CommitArgs {
  secretKey: Uint8Array; // 64-byte Solana keypair (seed || pubkey)
  /// Supplied by the caller's browser: public Solana RPC blocks Cloudflare
  /// egress, so the browser fetches the blockhash and submits the signed
  /// transaction, while the signing key never leaves the Worker.
  blockhash: string;
  programId: string;
  owner: string;
  policyPda: string;
  positionId: number;
  fingerprint: bigint;
  nStates: number;
  trancheBps: number;
}

/** Build and sign a CommitPolicy transaction; returns base64 for the
 *  caller to submit. */
export async function commitPolicy(args: CommitArgs): Promise<string> {
  const blockhash = args.blockhash;

  const keys = [args.owner, args.policyPda, SYSTEM_PROGRAM, args.programId];
  const accountKeys = keys.map(b58decode);

  // instruction data: tag | position_id u64 | fingerprint u64 | n_states u8 | tranche u16
  const data = new Uint8Array(20);
  const view = new DataView(data.buffer);
  data[0] = IX_COMMIT_POLICY;
  view.setBigUint64(1, BigInt(args.positionId), true);
  view.setBigUint64(9, args.fingerprint, true);
  data[17] = args.nStates;
  view.setUint16(18, args.trancheBps, true);

  const message: number[] = [
    1, // numRequiredSignatures
    0, // numReadonlySigned
    2, // numReadonlyUnsigned: system program + our program
    ...shortvec(accountKeys.length),
  ];
  for (const k of accountKeys) message.push(...k);
  message.push(...b58decode(blockhash));
  message.push(...shortvec(1)); // one instruction
  message.push(3); // program id index (our program)
  message.push(...shortvec(3), 0, 1, 2); // account indexes: owner, policy, system
  message.push(...shortvec(data.length), ...data);
  const messageBytes = Uint8Array.from(message);

  const key = await crypto.subtle.importKey(
    "pkcs8",
    pkcs8(args.secretKey),
    { name: "Ed25519" },
    false,
    ["sign"],
  );
  const sig = new Uint8Array(
    await crypto.subtle.sign({ name: "Ed25519" }, key, messageBytes),
  );

  const tx = Uint8Array.from([...shortvec(1), ...sig, ...messageBytes]);
  return btoa(String.fromCharCode(...tx));
}
