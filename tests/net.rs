#![allow(clippy::unwrap_used)]
#![allow(clippy::panic)]

use std::process::Command;

use testnet::testnet;
use testnet::NetConfig;
use testnet::NodeConfig;

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
        nodes: vec!["node1", "node2"],
    };
    testnet(config).unwrap();
}

#[test]
fn broadcast_one() {
    let config = NetConfig {
        main: |mut context| {
            let i = context.current_node_index();
            match i {
                0 => {
                    eprintln!("node {i} send start");
                    context.broadcast_one().send_string("ping".into())?;
                    eprintln!("node {i} send end");
                }
                _ => {
                    eprintln!("node {i} receive start");
                    let string = context.broadcast_one().recv_string()?;
                    eprintln!("node {i} receive end");
                    assert_eq!("ping", string);
                }
            };
            Ok(())
        },
        nodes: vec![NodeConfig::default(); 2],
    };
    testnet(config).unwrap();
}

#[test]
fn broadcast_all() {
    let config = NetConfig {
        main: |mut context| {
            let i = context.current_node_index();
            let all_data = context.broadcast_all_string(i.to_string())?;
            for (i, data) in all_data.into_iter().enumerate() {
                assert_eq!(i.to_string(), data);
            }
            Ok(())
        },
        nodes: vec![NodeConfig::default(); 2],
    };
    testnet(config).unwrap();
}

#[test]
#[should_panic]
fn handle_panic() {
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
        nodes: vec![NodeConfig::default(); 2],
    };
    testnet(config).unwrap();
}

struct UserConfig {
    tag: String,
}
