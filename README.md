# Simplicity Wallet

First wallet that uses Simplicity descriptors.

The latest research on Bitcoin smart contracts at your fingertips.

## Building

```
$ cargo build
```

## Installing Elements

We need [Elements Core with added Simplicity support](https://github.com/ElementsProject/elements/tree/simplicity).

Compile Elements manually using the [official instructions](https://github.com/ElementsProject/elements/blob/simplicity/doc/build-unix.md).

Or use the [provided nix shell](https://github.com/uncomputable/simpiwallet/blob/master/shell.nix). This will give you `elementsd` and `elements-cli`.

```
$ nix-shell
```

A slightly outdated version of Elements + Simplicity can be installed via nixpkgs.

```
$ nix-shell -p elementsd-simplicity
```

## Running

```
$ simpiwallet <SUBCOMMAND>
$ simpiwallet help
```

## Initializing the wallet

Generate the initial xpub and save it to disk.

```
$ simpiwallet new
```

Other operations will read and write from the same file.

**Secret keys are stored in plain on disk!** Don't forget, this is a research prototype. Use it on regtest or testnet, but never on mainnet!

## Running Elements

The wallet needs to communicate with Elements.

Feel free to use the [provided Elements configuration](https://github.com/uncomputable/simpiwallet/blob/master/elements.conf).

```
$ mkdir ~/.elements
$ cp elements.conf ~/.elements
```

Run elementsd.

```
$ elementsd
```

## Funding the wallet

Initially the wallet will not have any funds. You have to generate an address and send coins from another wallet.

Check how many coins are inside your Simplicity wallet. If there are enough, then you can skip the rest of this section.

```
$ simpiwallet getbalance
: <BALANCE>
```

Create an Elements wallet if you don't already have one.

```
$ elements-cli createwallet <WALLETNAME>
```

Or load your existing Elements wallet.

```
$ elements-cli loadwallet <WALLETNAME>
```

For technical reasons it is often necessary to rescan the blockchain.

```
$ elements-cli rescanblockchain
```

Check if your Elements wallet has funds. With the provided configuration, it should have 21 million bitcoin.

```
$ elements-cli getbalance
```

Generate an address for your Simplicity wallet.

```
$ simpiwallet getnewaddress
: <ADDRESS>
```

Send coins to your Simplicity wallet.

```
$ elements-cli sendtoaddress <ADDRESS> <AMOUNT>
```

Mine an Elements block to finalize the transaction.

```
$ elements-cli -generate 1
```

Now your Simplicity wallet should have a higher balance.

```
$ simpiwallet getfunds
: <HIGHER_BALANCE>
```

## Sending to address

Send coins to a given Elements address.

```
$ simpiwallet sendtoaddress <ADDRESS> <AMOUNT>
: <TXID>
```

The wallet will sign and broadcast the transaction immediately to Elements via RPC.

The returned transaction ID can be used to get the full transaction hex.

```
$ elements-cli getrawtransaction <TXID>
: <TXHEX>
```

Use [hal-simplicity](https://github.com/uncomputable/hal-simplicity) to inspect the transaction further.

```
$ hal-simplicity tx decode <TXHEX>
```

Don't forget to mine an Elements block to finalize the transaction.

```
$ elements-cli -generate 1
```
