use crate::utils::{build_ic_agent, build_management_canister, create_empty_canister, get_canister_wasm, install_wasm};
use crate::CanisterName;
use candid::Principal;
use ic_agent::identity::BasicIdentity;
use ic_agent::Identity;
use ic_utils::interfaces::ManagementCanister;
use ic_utils::Canister;
use types::{CanisterId, Version};

pub async fn create_and_install_service_canisters(identity: BasicIdentity, url: String, test_mode: bool) -> CanisterId {
    let principal = identity.sender().unwrap();
    let agent = build_ic_agent(url, identity).await;
    let management_canister = build_management_canister(&agent);

    let index_canister_id = create_empty_canister(&management_canister).await;

    println!("index canister id: {}", index_canister_id.to_string());

    install_service_canisters_impl(principal, &index_canister_id, &management_canister, test_mode).await;

    index_canister_id
}

pub async fn install_service_canisters(identity: BasicIdentity, url: String, index_canister_id: CanisterId, test_mode: bool) {
    let principal = identity.sender().unwrap();
    let agent = build_ic_agent(url, identity).await;
    let management_canister = build_management_canister(&agent);

    install_service_canisters_impl(principal, &index_canister_id, &management_canister, test_mode).await;
}

async fn install_service_canisters_impl(
    principal: Principal,
    index_canister_id: &CanisterId,
    management_canister: &Canister<'_, ManagementCanister>,
    test_mode: bool,
) {
    let version = Version::min();

    let index_canister_wasm = get_canister_wasm(CanisterName::Index, version, false);
    let bucket_canister_wasm = get_canister_wasm(CanisterName::Bucket, Version::min(), true);
    let index_init_args = index_canister::init::Args {
        service_principals: vec![principal],
        bucket_canister_wasm,
        wasm_version: Version::min(),
        test_mode,
    };

    install_wasm(
        management_canister,
        index_canister_id,
        &index_canister_wasm.module,
        index_init_args,
    )
    .await;

    println!("Canister wasms installed");
}
