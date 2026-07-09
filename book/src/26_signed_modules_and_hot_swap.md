# Chapter 26. Signed Modules and Hot Code Swap

## Goal

By the end of this chapter you will understand two things that can happen
to a finished program: it can be signed so its origin is provable, and it
can be swapped for new code while it runs.

## Signing: proving where a program came from

A bytecode file travels. It is compiled on one machine and may run on
another, far away. The machine that runs it has a question: is this the
genuine program, from the expected author, unaltered? A signature answers
that question.

The intended use is multi-party delivery. A studio compiles a program and
signs it with a private key that only the studio holds. A device in the
field holds the matching public key, and it refuses to run any program
whose signature does not check out against that key.

## Marking a program as signed

A program opts in with the `signed` modifier on its entry function:

```
signed fn main() -> Word {
    21 + 21
}
```

`signed` marks the program as one that must carry a valid signature
before it will load. It is allowed only on the entry function, and it
works on any of the three function kinds: `signed fn main`,
`signed yield main`, `signed loop main`.

## The signing flow

First, make a key pair:

```
keleusma keygen --seed studio.seed --public studio.pub
```

This writes two files. `studio.seed` is the private key, the secret the
studio guards. `studio.pub` is the public key, given to anyone who needs
to verify. The tool refuses to overwrite an existing key file, because a
key is a long-lived secret.

Save the program above as `app.kel`, then compile and sign it:

```
keleusma compile app.kel --signing-key studio.seed -o app.kel.bin
```

Run the signed bytecode, supplying the public key to verify against:

```
keleusma run app.kel.bin --verifying-key studio.pub
```

The output is `42`. The runtime checked the signature against
`studio.pub`, found it valid, and ran the program.

Run it without the key, and the program does not load:

```
error: verify_module_signature: InvalidSignature
```

A `signed` program will not run unless its signature checks out. It is a
sealed and signed score, and a player who trusts the seal. The signature
scheme is Ed25519, a standard and well-trusted one.

## Encryption: keeping the program confidential in transit

A signed bytecode artefact carries an integrity guarantee, but its
contents are still readable to anyone who intercepts it. For deployments
that require confidentiality as well, the bytecode can be encrypted to a
specific recipient at compile time.

The recipient generates an encryption keypair (X25519) and gives the
public half to whoever will produce the artefact:

```
keleusma keygen --kind encryption --seed device.seed --public device.pub
```

The compile step takes both a signing key and the recipient's public
encryption key:

```
keleusma compile app.kel \
    --signing-key studio.seed \
    --encryption-key device.pub \
    -o app.kel.bin
```

The recipient runs the artefact with both the verifying key and their
own decryption key:

```
keleusma run app.kel.bin \
    --verifying-key studio.pub \
    --decryption-key device.seed
```

Encryption is layered above signing. The signature covers the encrypted
body so an adversary cannot strip the encryption and substitute
cleartext without invalidating the signature. The encryption uses
X25519 key exchange combined with AES-256-GCM authenticated encryption.

The [`SECURITY_POLICY.md`](./SECURITY_POLICY.md) reference describes
both gates as deployment policies. The two policies are independent and
either may be activated through enrolled key stores in
platform-conventional directories.

## Hot code swap: changing the program while it runs

The second thing that can happen to a finished program is that it can be
replaced, while running, by a new one.

Recall RESET from Chapter 17: the boundary at the top of each cycle of a
`loop` function. RESET is also the moment at which a program can be
swapped. The running program finishes a cycle. At the RESET boundary, the
host installs new code in place of the old. The next cycle runs the new
program.

The conversation is not interrupted. The one thing that must stay the
same across a swap is the dialogue from Chapter 16, the agreed pair of
types exchanged at each `yield`. As long as the new program speaks the
same dialogue, the host keeps talking to it without a break. This is
swapping to a new arrangement at the next downbeat, without stopping the
band.

A program does not swap itself. The host performs the swap, at a RESET
boundary. Part VIII shows it directly: in the piano roll, pressing a key
swaps the running song for another, and that is a hot code swap.

## What you now know

- A bytecode file can be signed, so a running machine can prove its
  origin.
- The `signed` modifier on the entry function marks a program as
  requiring a valid signature.
- The flow is `keleusma keygen`, then `keleusma compile --signing-key`,
  then `keleusma run --verifying-key`.
- A signed bytecode artefact can additionally be encrypted to a
  specific recipient with `--encryption-key`. The recipient runs it
  with `--decryption-key`.
- Strict-mode deployment policies live in
  [`SECURITY_POLICY.md`](./SECURITY_POLICY.md).
- Hot code swap replaces a running program with new code at a RESET
  boundary, and the dialogue must stay the same across the swap.

That completes Part VII. The program is written, compiled, and shippable.
Part VIII puts a real one to work, and makes it audible.
