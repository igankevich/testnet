# testnet

[![Crates.io Version](https://img.shields.io/crates/v/testnet)](https://crates.io/crates/testnet)
[![Docs](https://docs.rs/testnet/badge.svg)](https://docs.rs/testnet)
[![dependency status](https://deps.rs/repo/github/igankevich/testnet/status.svg)](https://deps.rs/repo/github/igankevich/testnet)

A library that helps you test your distributed applications using a virtual network via Linux namespaces.


## Introduction

Certain distributed applications (VPNs, distributed key-value stores, blockchains etc.)
require a pre-configured network to run on.
Running on a loopback (127.0.0.0/8) is of course an option, but is it really enough?
More often than not you need to synchronize parallel processes to verify that the system
is in a valid expected state.
Doing so over loopback network requires some sort of a middleware that runs on top.
Testnet is such a middleware, and it also sets up a virtual network for you.

Testnet provides simple broadcast commands to synchronize parallel processes running on different network nodes and optionally share some data between all the nodes
(e.g. the endpoints and ports that are used by the applications, public and private keys that are needed by the test etc.).


## Examples


### Hello world

```rust
use testnet::testnet;
use testnet::NetConfig;
let config = NetConfig {
    main: |context| {
        let i = context.current_node_index();
        let node = context.current_node();
        eprintln!("hello from node {} name {:?}", i, node.name);
        Ok(())
    },
    nodes: vec!["node1", "node2"],
};
// will print
// 
// hello from node 0 name "node1"
// hello from node 1 name "node2"
testnet(config).unwrap();
```


### Ping between two nodes

```rust
use testnet::testnet;
use testnet::NetConfig;
use std::process::Command;
let config = NetConfig {
    main: |context| {
        let other = match context.current_node_name() {
            "node1" => "node2",
            "node2" => "node1",
            _ => unreachable!(),
        };
        Command::new("ping").args(["-c", "1", other]).status()?;
        Ok(())
    },
    nodes: vec!["node1", "node2"],
};
testnet(config).unwrap();
```
