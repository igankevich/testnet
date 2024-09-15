#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use std::process::Command;

use testnet::testnet;
use testnet::NetConfig;

#[test]
fn net2() {
    let user_config = [
        UserConfig {
            tag: "first".into(),
        },
        UserConfig {
            tag: "second".into(),
        },
    ];
    let config = NetConfig {
        main: |context| {
            let i = context.current_node_index();
            let node = context.current_node();
            eprintln!(
                "hello from node {} name {:?} tag {:?}",
                i, node.name, user_config[i].tag
            );
            Command::new("ip").args(["address"]).status()?;
            let j = match i {
                0 => 1,
                _ => 0,
            };
            Command::new("ping")
                .args(["-c", "1", &context.nodes()[j].ifaddr.addr().to_string()])
                .status()?;
            Ok(())
        },
        nodes: vec![Default::default(), Default::default()],
    };
    testnet(config).unwrap();
}

#[test]
fn ipc() {
    let config = NetConfig {
        main: |mut context| {
            let i = context.current_node_index();
            match i {
                0 => {
                    eprintln!("node {i} send start");
                    context.send("ping".into())?;
                    eprintln!("node {i} send end");
                }
                _ => {
                    eprintln!("node {i} receive start");
                    let data = context.recv()?;
                    eprintln!("node {i} receive end");
                    let string = String::from_utf8(data).unwrap();
                    assert_eq!("ping", string);
                }
            };
            Ok(())
        },
        nodes: vec![Default::default(), Default::default()],
    };
    testnet(config).unwrap();
}

#[test]
#[should_panic]
fn failure() {
    let config = NetConfig {
        main: |context| {
            let i = context.current_node_index();
            match i {
                0 => {
                    // ok
                }
                _ => {
                    panic!("this test should panic");
                }
            };
            Ok(())
        },
        nodes: vec![Default::default(), Default::default()],
    };
    testnet(config).unwrap();
}

struct UserConfig {
    tag: String,
}
