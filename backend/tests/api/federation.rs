use crate::helpers::{TestApp, TestUser, spawn_app};
use uuid::Uuid;

#[tokio::test]
async fn paired_laboratories_can_query_remote_public_inventory_data() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Federation Local Lab").await;
    let remote_laboratory_id = remote.create_laboratory("Federation Remote Lab").await;
    let remote_asset_id = seed_remote_asset(&remote, remote_laboratory_id).await;
    let internal_attachment_id =
        seed_remote_attachments(&remote, remote_laboratory_id, remote_asset_id).await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;

    let response = local
        .get_federation_assets(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["items"][0]["asset_id"], remote_asset_id.to_string());
    assert!(body["items"][0]["internal_notes"].is_null());

    let response = local
        .get_federation_attachment(remote_node_id, remote_laboratory_id, internal_attachment_id)
        .await;
    assert_eq!(response.status().as_u16(), 404);

    let remote_admin = TestUser::generate_with_user_type("lab_admin", Some(remote_laboratory_id));
    remote.store_user(&remote_admin).await;
    remote_admin.login(&remote).await;
    let response = remote
        .get_federation_guest_links(remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let links: serde_json::Value = response.json().await.unwrap();
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(links[0]["remote_user_id"], local_user.user_id.to_string());
}

#[tokio::test]
async fn federation_proxy_rejects_server_scoped_and_guest_users() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Federation Role Local").await;
    let remote_laboratory_id = remote.create_laboratory("Federation Role Remote").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    local.test_user.login(&local).await;
    let response = local
        .get_federation_assets(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 403);

    let guest = TestUser::generate_with_user_type("guest", Some(local_laboratory_id));
    local.store_user(&guest).await;
    guest.login(&local).await;
    let response = local
        .get_federation_assets(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 403);
}

#[tokio::test]
async fn laboratory_users_can_list_their_federation_trusts() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Federation List Local").await;
    let remote_laboratory_id = remote.create_laboratory("Federation List Remote").await;
    pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;

    let response = local.get_federation_trusts(local_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let trusts: serde_json::Value = response.json().await.unwrap();
    assert_eq!(trusts.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn inbound_federation_requires_a_valid_signature() {
    let app = spawn_app().await;
    let laboratory_id = app.create_laboratory("Unsigned Federation Lab").await;
    let response = app
        .get_api_path(&format!(
            "/federation/inbound/laboratories/{laboratory_id}/assets"
        ))
        .await;
    assert_eq!(response.status().as_u16(), 401);
}

#[tokio::test]
async fn federation_guest_link_can_be_merged_into_existing_guest() {
    let local = spawn_app().await;
    let remote = spawn_app().await;
    let local_laboratory_id = local.create_laboratory("Federation Merge Local").await;
    let remote_laboratory_id = remote.create_laboratory("Federation Merge Remote").await;
    let remote_node_id =
        pair_laboratories(&local, local_laboratory_id, &remote, remote_laboratory_id).await;

    let local_user = TestUser::generate_with_user_type("user", Some(local_laboratory_id));
    local.store_user(&local_user).await;
    local_user.login(&local).await;
    let response = local
        .get_federation_assets(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);

    let remote_admin = TestUser::generate_with_user_type("lab_admin", Some(remote_laboratory_id));
    remote.store_user(&remote_admin).await;
    let existing_guest = TestUser::generate_with_user_type("guest", Some(remote_laboratory_id));
    remote.store_user(&existing_guest).await;
    remote_admin.login(&remote).await;

    let response = remote
        .get_federation_guest_links(remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let links: serde_json::Value = response.json().await.unwrap();
    let link_id: Uuid = links[0]["link_id"].as_str().unwrap().parse().unwrap();
    let response = remote
        .merge_federation_guest_link(
            remote_laboratory_id,
            link_id,
            &serde_json::json!({ "target_guest_user_id": existing_guest.user_id }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let merged: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        merged["local_guest_user_id"],
        existing_guest.user_id.to_string()
    );

    local_user.login(&local).await;
    let response = local
        .get_federation_assets(remote_node_id, remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    remote_admin.login(&remote).await;
    let response = remote
        .get_federation_guest_links(remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let links: serde_json::Value = response.json().await.unwrap();
    assert_eq!(links.as_array().unwrap().len(), 1);
    assert_eq!(
        links[0]["local_guest_user_id"],
        existing_guest.user_id.to_string()
    );
}

async fn pair_laboratories(
    local: &TestApp,
    local_laboratory_id: Uuid,
    remote: &TestApp,
    remote_laboratory_id: Uuid,
) -> Uuid {
    let remote_admin = TestUser::generate_with_user_type("lab_admin", Some(remote_laboratory_id));
    remote.store_user(&remote_admin).await;
    remote_admin.login(remote).await;
    let response = remote
        .post_federation_pairing_code(remote_laboratory_id)
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let pairing: serde_json::Value = response.json().await.unwrap();
    let pairing_code = pairing["pairing_code"].as_str().unwrap();

    let local_admin = TestUser::generate_with_user_type("lab_admin", Some(local_laboratory_id));
    local.store_user(&local_admin).await;
    local_admin.login(local).await;
    let response = local
        .post_federation_trust(
            local_laboratory_id,
            &serde_json::json!({
                "remote_base_url": remote.address,
                "remote_laboratory_id": remote_laboratory_id,
                "pairing_code": pairing_code
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let trust: serde_json::Value = response.json().await.unwrap();
    let response = local.get_federation_trusts(local_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let trusts: serde_json::Value = response.json().await.unwrap();
    assert_eq!(trusts.as_array().unwrap().len(), 1);
    trust["remote_node_id"].as_str().unwrap().parse().unwrap()
}

async fn seed_remote_asset(app: &TestApp, laboratory_id: Uuid) -> Uuid {
    app.test_user.login(app).await;
    let unit_id = app.unit_id("pcs").await;
    let response = app
        .post_asset(
            laboratory_id,
            &serde_json::json!({
                "tracking_mode": "quantity",
                "name": "Federated Reagent",
                "default_unit_id": unit_id,
                "public_notes": "shared",
                "internal_notes": "secret"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    body["asset_id"].as_str().unwrap().parse().unwrap()
}

async fn seed_remote_attachments(app: &TestApp, laboratory_id: Uuid, asset_id: Uuid) -> Uuid {
    app.test_user.login(app).await;
    let upload = app
        .upload_attachment(
            laboratory_id,
            "internal.txt",
            "text/plain",
            b"secret".to_vec(),
        )
        .await;
    assert_eq!(upload.status().as_u16(), 201);
    let upload: serde_json::Value = upload.json().await.unwrap();
    let response = app
        .post_asset_attachment(
            asset_id,
            &serde_json::json!({
                "upload_id": upload["upload_id"],
                "visibility": "internal"
            }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let attachment: serde_json::Value = response.json().await.unwrap();
    attachment["attachment_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap()
}
