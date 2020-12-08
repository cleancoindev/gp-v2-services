use ethcontract_generate::{Address, Builder};
use maplit::hashmap;
use std::str::FromStr;
use std::{collections::HashMap, env, path::Path};

#[path = "src/paths.rs"]
mod paths;

fn main() {
    // NOTE: This is a workaround for `rerun-if-changed` directives for
    // non-existant files cause the crate's build unit to get flagged for a
    // rebuild if any files in the workspace change.
    //
    // See:
    // - https://github.com/rust-lang/cargo/issues/6003
    // - https://doc.rust-lang.org/cargo/reference/build-scripts.html#cargorerun-if-changedpath
    println!("cargo:rerun-if-changed=build.rs");

    generate_contract("IERC20", hashmap! {});
    generate_contract(
        "IUniswapV2Router02",
        hashmap! {4 => Address::from_str("0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D").unwrap()},
    );
    generate_contract(
        "GPv2Settlement",
        hashmap! {4 => Address::from_str("0x828229A8432A89B8624B6AF91eC0BB65b9517156").unwrap()},
    );
}

fn generate_contract(name: &str, deployments: HashMap<u32, Address>) {
    let artifact = paths::contract_artifacts_dir().join(format!("{}.json", name));
    let dest = env::var("OUT_DIR").unwrap();

    println!("cargo:rerun-if-changed={}", artifact.display());
    let builder = Builder::new(artifact)
        .with_contract_name_override(Some(name))
        .with_visibility_modifier(Some("pub"))
        .add_event_derive("serde::Deserialize")
        .add_event_derive("serde::Serialize");
    for (network, address) in deployments.into_iter() {
        builder = builder.add_deployment(network, address);
    }

    builder
        .generate()
        .unwrap()
        .write_to_file(Path::new(&dest).join(format!("{}.rs", name)))
        .unwrap();
}
