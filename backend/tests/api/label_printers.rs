use crate::helpers::{TestApp, TestUser, spawn_app};
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

/// Raster rows are 90 bytes wide on every QL-800 family printer.
const BYTES_PER_ROW: usize = 90;
const DIE_CUT_62X29_WIDTH_DOTS: u16 = 696;
const DIE_CUT_62X29_LENGTH_DOTS: u16 = 271;

/// A socket that plays the part of a QL-820NWBc.
///
/// It answers the status request, then swallows the job so the bytes the server
/// actually put on the wire can be asserted.
struct FakePrinter {
    port: u16,
    received: tokio::task::JoinHandle<Vec<u8>>,
}

impl FakePrinter {
    /// `media` is the (kind, width_mm, length_mm) triple the printer claims to
    /// have loaded.
    async fn listening(media: (u8, u8, u8)) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("fake printer binds");
        let port = listener.local_addr().expect("fake printer has a port").port();

        let mut block = [0u8; 32];
        block[0] = 0x80;
        block[1] = 0x20;
        block[10] = media.1;
        block[11] = media.0;
        block[17] = media.2;

        let received = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("a connection arrives");
            // Replying up front is fine: the status request is already in
            // flight, and TCP buffers the answer until the server reads it.
            stream.write_all(&block).await.expect("status is writable");
            stream.flush().await.expect("status flushes");

            let mut received = Vec::new();
            stream
                .read_to_end(&mut received)
                .await
                .expect("the job is readable");
            received
        });

        Self { port, received }
    }

    async fn bytes(self) -> Vec<u8> {
        tokio::time::timeout(Duration::from_secs(5), self.received)
            .await
            .expect("the fake printer finished reading")
            .expect("the fake printer task did not panic")
    }
}

fn blank_bitmap(width_dots: u16, height_dots: u16) -> String {
    let row_bytes = usize::from(width_dots).div_ceil(8);
    STANDARD.encode(vec![0u8; row_bytes * usize::from(height_dots)])
}

fn printer_body(port: u16) -> serde_json::Value {
    serde_json::json!({
        "name": format!("Printer {}", Uuid::new_v4()),
        "host": "127.0.0.1",
        "port": port,
        "media_kind": "die_cut",
        "media_width_mm": 62,
        "media_length_mm": 29,
    })
}

async fn create_printer(app: &TestApp, laboratory_id: Uuid, port: u16) -> Uuid {
    let response = app
        .post_label_printer(laboratory_id, &printer_body(port))
        .await;
    assert_eq!(response.status().as_u16(), 201);
    let body: serde_json::Value = response.json().await.unwrap();
    Uuid::parse_str(body["printer_id"].as_str().unwrap()).unwrap()
}

fn single_page() -> serde_json::Value {
    serde_json::json!({
        "pages": [{
            "width_dots": DIE_CUT_62X29_WIDTH_DOTS,
            "height_dots": DIE_CUT_62X29_LENGTH_DOTS,
            "bitmap_base64": blank_bitmap(DIE_CUT_62X29_WIDTH_DOTS, DIE_CUT_62X29_LENGTH_DOTS),
        }]
    })
}

/// Splits the status exchange off the front of what the printer received,
/// leaving just the print job.
fn job_bytes(received: &[u8]) -> &[u8] {
    // 400 invalidate zeros + ESC @ + ESC i S
    const STATUS_REQUEST_LEN: usize = 400 + 2 + 3;
    assert!(
        received.len() > STATUS_REQUEST_LEN,
        "the printer should have received a job after the status request"
    );
    &received[STATUS_REQUEST_LEN..]
}

#[tokio::test]
async fn label_printer_crud_is_laboratory_scoped_and_admin_only() {
    let app = spawn_app().await;
    let own_laboratory_id = app.create_laboratory("Printer Own Lab").await;
    let other_laboratory_id = app.create_laboratory("Printer Other Lab").await;

    let response = app.get_label_printers(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 401);

    app.test_user.login(&app).await;
    let own_printer_id = create_printer(&app, own_laboratory_id, 9100).await;
    let other_printer_id = create_printer(&app, other_laboratory_id, 9100).await;

    let response = app.get_label_printer(own_printer_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["laboratory_id"], own_laboratory_id.to_string());
    assert_eq!(body["media_kind"], "die_cut");
    // The client renders the bitmap, so it is told the exact dot dimensions.
    assert_eq!(body["layout"]["printable_width_dots"], 696);
    assert_eq!(body["layout"]["printable_length_dots"], 271);
    assert_eq!(body["layout"]["dpi"], 300);

    // A laboratory administrator sees only their own laboratory's printers.
    let lab_admin = TestUser::generate_with_user_type("lab_admin", Some(own_laboratory_id));
    app.store_user(&lab_admin).await;
    lab_admin.login(&app).await;

    let response = app.get_label_printers(own_laboratory_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let printers: serde_json::Value = response.json().await.unwrap();
    assert_eq!(printers.as_array().unwrap().len(), 1);

    assert_eq!(
        app.get_label_printer(other_printer_id).await.status().as_u16(),
        404
    );
    assert_eq!(app.get_label_printer(Uuid::new_v4()).await.status().as_u16(), 404);

    // A regular user may look at printers but not reconfigure them.
    let regular_user = TestUser::generate_with_user_type("user", Some(own_laboratory_id));
    app.store_user(&regular_user).await;
    regular_user.login(&app).await;

    assert_eq!(
        app.get_label_printers(own_laboratory_id).await.status().as_u16(),
        200
    );
    assert_eq!(app.get_label_printer(own_printer_id).await.status().as_u16(), 200);
    assert_eq!(
        app.post_label_printer(own_laboratory_id, &printer_body(9100))
            .await
            .status()
            .as_u16(),
        403
    );
    assert_eq!(
        app.patch_label_printer(own_printer_id, &serde_json::json!({ "auto_cut": false }))
            .await
            .status()
            .as_u16(),
        403
    );
    assert_eq!(
        app.delete_label_printer(own_printer_id).await.status().as_u16(),
        403
    );

    // Guests cannot see printers at all, so printing is closed to them too.
    let guest = TestUser::generate_with_user_type("guest", Some(own_laboratory_id));
    app.store_user(&guest).await;
    guest.login(&app).await;

    assert_eq!(
        app.get_label_printers(own_laboratory_id).await.status().as_u16(),
        403
    );
    assert_eq!(app.get_label_printer(own_printer_id).await.status().as_u16(), 404);
    assert_eq!(
        app.post_label_printer_print(own_printer_id, &single_page())
            .await
            .status()
            .as_u16(),
        404
    );
}

#[tokio::test]
async fn label_printer_updates_and_deletes_are_recorded() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Update Lab").await;
    let printer_id = create_printer(&app, laboratory_id, 9100).await;

    // Media moves as one value: switching to continuous clears the length.
    let response = app
        .patch_label_printer(
            printer_id,
            &serde_json::json!({ "media_kind": "continuous", "media_width_mm": 62 }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["media_kind"], "continuous");
    assert!(body["media_length_mm"].is_null());
    assert_eq!(body["layout"]["printable_length_dots"], 0);

    // Half a media change is refused rather than applied.
    let response = app
        .patch_label_printer(printer_id, &serde_json::json!({ "media_kind": "die_cut" }))
        .await;
    assert_eq!(response.status().as_u16(), 400);

    // Unsupported stock never reaches the database.
    let response = app
        .patch_label_printer(
            printer_id,
            &serde_json::json!({ "media_kind": "die_cut", "media_width_mm": 62, "media_length_mm": 30 }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 400);

    let actions: Vec<String> = sqlx::query_scalar(
        "SELECT action FROM audit_logs WHERE resource_type = 'label_printer' AND resource_id = $1 ORDER BY created_at",
    )
    .bind(printer_id)
    .fetch_all(&app.db_pool)
    .await
    .expect("Failed to read audit log.");
    assert_eq!(actions, vec!["create", "update"]);

    assert_eq!(app.delete_label_printer(printer_id).await.status().as_u16(), 204);
    assert_eq!(app.get_label_printer(printer_id).await.status().as_u16(), 404);
}

#[tokio::test]
async fn label_printer_registration_validates_the_address_and_media() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Validation Lab").await;

    let rejected = [
        serde_json::json!({ "name": "A", "host": "", "media_kind": "die_cut", "media_width_mm": 62, "media_length_mm": 29 }),
        // The port has its own field, so a host may not smuggle one in.
        serde_json::json!({ "name": "B", "host": "192.168.1.5:9100", "media_kind": "die_cut", "media_width_mm": 62, "media_length_mm": 29 }),
        serde_json::json!({ "name": "C", "host": "http://192.168.1.5", "media_kind": "die_cut", "media_width_mm": 62, "media_length_mm": 29 }),
        // Privileged ports are refused so a registration cannot probe the host.
        serde_json::json!({ "name": "D", "host": "192.168.1.5", "port": 22, "media_kind": "die_cut", "media_width_mm": 62, "media_length_mm": 29 }),
        serde_json::json!({ "name": "E", "host": "192.168.1.5", "media_kind": "roll", "media_width_mm": 62 }),
        serde_json::json!({ "name": "F", "host": "192.168.1.5", "media_kind": "die_cut", "media_width_mm": 62 }),
        serde_json::json!({ "name": "G", "host": "192.168.1.5", "media_kind": "continuous", "media_width_mm": 62, "media_length_mm": 29 }),
        serde_json::json!({ "name": "H", "host": "192.168.1.5", "media_kind": "continuous", "media_width_mm": 45 }),
    ];

    for body in rejected {
        let response = app.post_label_printer(laboratory_id, &body).await;
        assert_eq!(
            response.status().as_u16(),
            400,
            "expected {body} to be rejected"
        );
    }

    // Two printers in one laboratory cannot share a name.
    let body = printer_body(9100);
    assert_eq!(
        app.post_label_printer(laboratory_id, &body).await.status().as_u16(),
        201
    );
    assert_eq!(
        app.post_label_printer(laboratory_id, &body).await.status().as_u16(),
        409
    );
}

#[tokio::test]
async fn printing_sends_a_single_job_and_records_an_audit_entry() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Print Lab").await;

    let printer = FakePrinter::listening((0x0B, 62, 29)).await;
    let printer_id = create_printer(&app, laboratory_id, printer.port).await;

    // Two labels, two copies each: one job, four pages.
    let page = serde_json::json!({
        "width_dots": DIE_CUT_62X29_WIDTH_DOTS,
        "height_dots": DIE_CUT_62X29_LENGTH_DOTS,
        "bitmap_base64": blank_bitmap(DIE_CUT_62X29_WIDTH_DOTS, DIE_CUT_62X29_LENGTH_DOTS),
    });
    let response = app
        .post_label_printer_print(
            printer_id,
            &serde_json::json!({ "pages": [page, page], "copies": 2 }),
        )
        .await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["labels_printed"], 4);

    let received = printer.bytes().await;
    let job = job_bytes(&received);

    // The job resets the printer and switches it into raster mode.
    assert_eq!(&job[..400], &vec![0u8; 400][..]);
    assert_eq!(&job[400..409], &[0x1B, b'@', 0x1B, b'i', b'a', 0x01, 0x1B, b'i', b'S']);

    // Print information describes the loaded stock and the page length.
    let print_info = job
        .windows(3)
        .position(|w| w == [0x1B, b'i', b'z'])
        .expect("print information is emitted");
    assert_eq!(job[print_info + 3], 0xCE);
    assert_eq!(job[print_info + 4], 0x0B, "die-cut media type");
    assert_eq!(job[print_info + 5], 62);
    assert_eq!(job[print_info + 6], 29);
    assert_eq!(
        &job[print_info + 7..print_info + 11],
        &u32::from(DIE_CUT_62X29_LENGTH_DOTS).to_le_bytes()
    );

    // Four pages, and only the last one feeds the tape out.
    assert_eq!(job.iter().filter(|&&b| b == 0x0C).count(), 3);
    assert_eq!(job.last(), Some(&0x1A));

    // Every raster row decompresses back to a full print-head line.
    let mut rows = 0;
    let mut index = 0;
    while index + 3 <= job.len() {
        if job[index] == b'g' && job[index + 1] == 0x00 {
            let length = usize::from(job[index + 2]);
            rows += 1;
            index += 3 + length;
            continue;
        }
        index += 1;
    }
    assert_eq!(rows, usize::from(DIE_CUT_62X29_LENGTH_DOTS) * 4);

    let details: serde_json::Value = sqlx::query_scalar(
        "SELECT details FROM audit_logs WHERE action = 'print' AND resource_id = $1",
    )
    .bind(printer_id)
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to read print audit log.");
    assert_eq!(details["labels_printed"], 4);
    assert_eq!(details["copies"], 2);
    assert_eq!(details["pages"], 2);
}

#[tokio::test]
async fn printing_is_refused_when_the_loaded_stock_is_not_what_the_label_expects() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Mismatch Lab").await;

    // The printer is configured for 62x29 but is actually loaded with 62x100.
    let printer = FakePrinter::listening((0x0B, 62, 100)).await;
    let printer_id = create_printer(&app, laboratory_id, printer.port).await;

    let response = app
        .post_label_printer_print(printer_id, &single_page())
        .await;
    assert_eq!(response.status().as_u16(), 409);
    let message = response.text().await.unwrap();
    assert!(
        message.contains("62x100") && message.contains("62x29"),
        "the error should name both sizes, got: {message}"
    );

    // Nothing was printed, so nothing was logged.
    let prints: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_logs WHERE action = 'print' AND resource_id = $1",
    )
    .bind(printer_id)
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to count print audit logs.");
    assert_eq!(prints, 0);
}

#[tokio::test]
async fn printing_is_refused_when_the_printer_reports_a_fault() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Fault Lab").await;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("fake printer binds");
    let port = listener.local_addr().expect("fake printer has a port").port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("a connection arrives");
        let mut block = [0u8; 32];
        block[8] = 0b0000_0001; // no media
        block[9] = 0b0001_0000; // cover open
        block[10] = 62;
        block[11] = 0x0B;
        block[17] = 29;
        let _ = stream.write_all(&block).await;
        let _ = stream.flush().await;
        let mut sink = Vec::new();
        let _ = stream.read_to_end(&mut sink).await;
    });

    let printer_id = create_printer(&app, laboratory_id, port).await;
    let response = app
        .post_label_printer_print(printer_id, &single_page())
        .await;
    assert_eq!(response.status().as_u16(), 409);
    let message = response.text().await.unwrap();
    assert!(
        message.contains("no_media") && message.contains("cover_open"),
        "the error should name the faults, got: {message}"
    );
}

#[tokio::test]
async fn print_requests_are_validated_before_anything_reaches_the_printer() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Reject Lab").await;
    // Nothing is listening: a request that reaches the network would fail with
    // 502, so a 400 proves it was rejected before the connection was attempted.
    let printer_id = create_printer(&app, laboratory_id, 9100).await;

    let width = DIE_CUT_62X29_WIDTH_DOTS;
    let height = DIE_CUT_62X29_LENGTH_DOTS;
    let rejected = [
        // No pages at all.
        serde_json::json!({ "pages": [] }),
        // Width does not match what the loaded stock can print.
        serde_json::json!({ "pages": [{
            "width_dots": 306, "height_dots": height, "bitmap_base64": blank_bitmap(306, height),
        }]}),
        // Die-cut labels have a fixed length.
        serde_json::json!({ "pages": [{
            "width_dots": width, "height_dots": 300, "bitmap_base64": blank_bitmap(width, 300),
        }]}),
        // The bitmap is not the size the dimensions imply.
        serde_json::json!({ "pages": [{
            "width_dots": width, "height_dots": height, "bitmap_base64": blank_bitmap(width, 10),
        }]}),
        // Not base64 at all.
        serde_json::json!({ "pages": [{
            "width_dots": width, "height_dots": height, "bitmap_base64": "not base64!",
        }]}),
        // Copy counts are bounded.
        serde_json::json!({ "pages": [{
            "width_dots": width, "height_dots": height, "bitmap_base64": blank_bitmap(width, height),
        }], "copies": 0 }),
        serde_json::json!({ "pages": [{
            "width_dots": width, "height_dots": height, "bitmap_base64": blank_bitmap(width, height),
        }], "copies": 21 }),
    ];

    for body in rejected {
        let response = app.post_label_printer_print(printer_id, &body).await;
        assert_eq!(
            response.status().as_u16(),
            400,
            "expected the request to be rejected before dialling the printer"
        );
    }
}

#[tokio::test]
async fn printer_status_reports_the_loaded_media_and_whether_it_matches() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Status Lab").await;

    let printer = FakePrinter::listening((0x0B, 62, 29)).await;
    let printer_id = create_printer(&app, laboratory_id, printer.port).await;

    let response = app.get_label_printer_status(printer_id).await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["media_kind"], "die_cut");
    assert_eq!(body["media_width_mm"], 62);
    assert_eq!(body["media_length_mm"], 29);
    assert_eq!(body["ready"], true);
    assert_eq!(body["media_matches_configuration"], true);
    assert_eq!(body["faults"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn printer_status_reports_an_unreachable_printer_as_a_gateway_error() {
    let app = spawn_app().await;
    app.test_user.login(&app).await;
    let laboratory_id = app.create_laboratory("Printer Offline Lab").await;
    let printer_id = create_printer(&app, laboratory_id, 9100).await;

    let response = app.get_label_printer_status(printer_id).await;
    assert_eq!(response.status().as_u16(), 502);
}

#[tokio::test]
async fn instance_identity_reports_the_node_id_and_web_origin() {
    let app = spawn_app().await;

    let response = app.get_instance_identity().await;
    assert_eq!(response.status().as_u16(), 401);

    app.test_user.login(&app).await;
    let response = app.get_instance_identity().await;
    assert_eq!(response.status().as_u16(), 200);
    let body: serde_json::Value = response.json().await.unwrap();

    let node_id: Uuid = sqlx::query_scalar("SELECT node_id FROM federation_local_nodes")
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to read the local node id.");
    assert_eq!(body["node_id"], node_id.to_string());

    let public_web_url = body["public_web_url"].as_str().unwrap();
    assert!(!public_web_url.is_empty());
    assert!(
        !public_web_url.ends_with('/'),
        "the origin should be normalised so clients can append a path"
    );
}
