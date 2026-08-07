pragma circom 2.1.6;

// GreenProof - supplier commitment sub-circuit
//
// Proves that a private (supplierId, supplierSecret) pair hashes, via
// Poseidon, to a public commitment the supplier published/registered
// out-of-band (e.g. shared with the buyer once, off-chain, before any
// proof is generated). This lets a verifier confirm "this proof was
// produced by the holder of the secret behind commitment C" without
// learning supplierId or supplierSecret.
//
// This is a standard commitment-opening pattern (commit = H(id, secret)),
// NOT a novel cryptographic primitive.

include "circomlib/circuits/poseidon.circom";

template SupplierCommitment() {
    signal input supplierId;       // private
    signal input supplierSecret;   // private
    signal output commitment;      // to be constrained equal to a public input by the caller

    component hasher = Poseidon(2);
    hasher.inputs[0] <== supplierId;
    hasher.inputs[1] <== supplierSecret;

    commitment <== hasher.out;
}
