//! Talking to a label printer over its raw TCP port.
//!
//! A printer normally lives on the same LAN as the server, so unlike the
//! federation client this deliberately *allows* private addresses. What it does
//! block is the set of addresses that would turn a printer registration into a
//! way to reach the host itself or the cloud metadata service.
use super::raster::status_request;
use super::status::{PrinterStatus, STATUS_BLOCK_LEN, StatusError};
use super::{INVALIDATE_BYTES, MIN_PRINTER_PORT};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Where a configured printer can be reached.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrinterEndpoint {
    pub host: String,
    pub port: u16,
}

/// How strictly printer addresses are policed.
///
/// Loopback is refused in production because it would let a printer
/// registration point back at this server, but tests and local development need
/// to stand a fake printer up on 127.0.0.1.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AddressPolicy {
    pub allow_loopback: bool,
}

impl AddressPolicy {
    pub const STRICT: Self = Self {
        allow_loopback: false,
    };
    pub const PERMISSIVE: Self = Self {
        allow_loopback: true,
    };
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Printer address {0} is not allowed")]
    BlockedAddress(String),
    #[error("Printer port {0} is not allowed; use the printer's raw printing port (usually 9100)")]
    BlockedPort(u16),
    #[error("Could not resolve printer host {host}")]
    Unresolvable { host: String },
    #[error("Could not reach the printer at {endpoint}")]
    Unreachable {
        endpoint: String,
        #[source]
        source: std::io::Error,
    },
    #[error("The printer stopped responding")]
    Io(#[from] std::io::Error),
    #[error("The printer did not respond within the time allowed")]
    Timeout,
    #[error(transparent)]
    Status(#[from] StatusError),
}

impl std::fmt::Display for PrinterEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.host, self.port)
    }
}

/// Rejects addresses a printer would never legitimately live at.
///
/// Link-local covers the cloud metadata endpoint at 169.254.169.254. Private
/// ranges are allowed on purpose — that is where printers actually are.
pub fn validate_printer_ip(address: IpAddr, policy: AddressPolicy) -> Result<(), TransportError> {
    if policy.allow_loopback && address.is_loopback() {
        return Ok(());
    }

    let blocked = match address {
        IpAddr::V4(v4) => is_blocked_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(mapped) => is_blocked_v4(mapped),
            None => is_blocked_v6(v6),
        },
    };

    if blocked {
        return Err(TransportError::BlockedAddress(address.to_string()));
    }
    Ok(())
}

fn is_blocked_v4(address: Ipv4Addr) -> bool {
    address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_broadcast()
        || address.is_multicast()
        || address.is_documentation()
}

fn is_blocked_v6(address: Ipv6Addr) -> bool {
    // fe80::/10 is link-local; `is_unicast_link_local` is not yet stable.
    let link_local = (address.segments()[0] & 0xffc0) == 0xfe80;
    address.is_loopback() || address.is_unspecified() || address.is_multicast() || link_local
}

/// Resolves the endpoint and checks every address it maps to.
///
/// All addresses must pass, so a hostname cannot smuggle a blocked one through
/// alongside an allowed one.
async fn resolve(
    endpoint: &PrinterEndpoint,
    policy: AddressPolicy,
) -> Result<Vec<SocketAddr>, TransportError> {
    if endpoint.port < MIN_PRINTER_PORT {
        return Err(TransportError::BlockedPort(endpoint.port));
    }

    let addresses: Vec<_> = tokio::net::lookup_host((endpoint.host.as_str(), endpoint.port))
        .await
        .map_err(|_| TransportError::Unresolvable {
            host: endpoint.host.clone(),
        })?
        .collect();

    if addresses.is_empty() {
        return Err(TransportError::Unresolvable {
            host: endpoint.host.clone(),
        });
    }

    for address in &addresses {
        validate_printer_ip(address.ip(), policy)?;
    }

    Ok(addresses)
}

async fn connect(
    endpoint: &PrinterEndpoint,
    policy: AddressPolicy,
) -> Result<TcpStream, TransportError> {
    let addresses = resolve(endpoint, policy).await?;
    let mut last_error = None;

    for address in addresses {
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last_error = Some(error),
            Err(_) => last_error = None,
        }
    }

    match last_error {
        Some(source) => Err(TransportError::Unreachable {
            endpoint: endpoint.to_string(),
            source,
        }),
        None => Err(TransportError::Timeout),
    }
}

/// An open session with a printer.
///
/// Status and job share one connection so that what the printer reported is
/// still true when the job lands: checking on a separate connection would leave
/// a window in which the roll could be swapped.
pub struct PrinterConnection {
    stream: TcpStream,
}

/// Opens a session, applying the address policy first.
pub async fn open(
    endpoint: &PrinterEndpoint,
    policy: AddressPolicy,
) -> Result<PrinterConnection, TransportError> {
    Ok(PrinterConnection {
        stream: connect(endpoint, policy).await?,
    })
}

impl PrinterConnection {
    /// Asks the printer what media it has loaded and whether it is ready.
    pub async fn request_status(&mut self) -> Result<PrinterStatus, TransportError> {
        // The printer only reports status once it has been reset out of
        // whatever state a previous job left it in.
        let mut request = Vec::with_capacity(INVALIDATE_BYTES + 5);
        request.extend(std::iter::repeat_n(0x00, INVALIDATE_BYTES));
        request.extend_from_slice(&[0x1B, b'@']);
        request.extend_from_slice(&status_request());
        self.write(&request).await?;

        let mut block = [0u8; STATUS_BLOCK_LEN];
        tokio::time::timeout(READ_TIMEOUT, self.stream.read_exact(&mut block))
            .await
            .map_err(|_| TransportError::Timeout)??;

        Ok(PrinterStatus::parse(&block)?)
    }

    /// Streams an encoded job. The job carries its own reset preamble, so it is
    /// safe to send after a status exchange.
    pub async fn write_job(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        self.write(payload).await
    }

    async fn write(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        tokio::time::timeout(WRITE_TIMEOUT, async {
            self.stream.write_all(payload).await?;
            self.stream.flush().await
        })
        .await
        .map_err(|_| TransportError::Timeout)??;
        Ok(())
    }
}

/// Streams an encoded job to the printer over a connection of its own.
pub async fn send_job(
    endpoint: &PrinterEndpoint,
    policy: AddressPolicy,
    payload: &[u8],
) -> Result<(), TransportError> {
    open(endpoint, policy).await?.write_job(payload).await
}

/// Asks the printer what media it has loaded, without printing anything.
pub async fn query_status(
    endpoint: &PrinterEndpoint,
    policy: AddressPolicy,
) -> Result<PrinterStatus, TransportError> {
    open(endpoint, policy).await?.request_status().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blocked(address: &str) -> bool {
        validate_printer_ip(
            address.parse().expect("test address parses"),
            AddressPolicy::STRICT,
        )
        .is_err()
    }

    #[test]
    fn allows_the_private_ranges_printers_live_in() {
        assert!(!blocked("192.168.1.50"));
        assert!(!blocked("10.0.0.7"));
        assert!(!blocked("172.16.4.9"));
    }

    #[test]
    fn allows_ordinary_public_addresses() {
        assert!(!blocked("8.8.8.8"));
    }

    #[test]
    fn blocks_loopback_so_a_printer_cannot_point_at_this_server() {
        assert!(blocked("127.0.0.1"));
        assert!(blocked("127.1.2.3"));
        assert!(blocked("::1"));
    }

    #[test]
    fn allows_loopback_only_when_the_policy_says_so() {
        let permissive = validate_printer_ip(
            "127.0.0.1".parse().expect("address parses"),
            AddressPolicy::PERMISSIVE,
        );
        assert!(permissive.is_ok());

        // The relaxation covers loopback and nothing else.
        let metadata = validate_printer_ip(
            "169.254.169.254".parse().expect("address parses"),
            AddressPolicy::PERMISSIVE,
        );
        assert!(metadata.is_err());
    }

    #[test]
    fn blocks_the_cloud_metadata_endpoint() {
        assert!(blocked("169.254.169.254"));
        assert!(blocked("169.254.0.1"));
    }

    #[test]
    fn blocks_unspecified_broadcast_and_multicast() {
        assert!(blocked("0.0.0.0"));
        assert!(blocked("255.255.255.255"));
        assert!(blocked("224.0.0.1"));
        assert!(blocked("::"));
        assert!(blocked("ff02::1"));
    }

    #[test]
    fn blocks_ipv6_link_local() {
        assert!(blocked("fe80::1"));
        assert!(blocked("febf::1"));
        // fec0:: is outside fe80::/10 and is not blocked as link-local.
        assert!(!blocked("fec0::1"));
    }

    #[test]
    fn blocks_ipv4_mapped_addresses_that_would_be_blocked_as_ipv4() {
        assert!(blocked("::ffff:127.0.0.1"));
        assert!(blocked("::ffff:169.254.169.254"));
        assert!(!blocked("::ffff:192.168.1.50"));
    }

    #[tokio::test]
    async fn rejects_privileged_ports() {
        let endpoint = PrinterEndpoint {
            host: "192.168.1.50".into(),
            port: 22,
        };
        assert!(matches!(
            resolve(&endpoint, AddressPolicy::STRICT).await,
            Err(TransportError::BlockedPort(22))
        ));
    }

    #[tokio::test]
    async fn rejects_a_blocked_host_before_connecting() {
        let endpoint = PrinterEndpoint {
            host: "127.0.0.1".into(),
            port: 9100,
        };
        assert!(matches!(
            resolve(&endpoint, AddressPolicy::STRICT).await,
            Err(TransportError::BlockedAddress(_))
        ));
    }

    /// Stands up a socket that plays the part of a printer.
    async fn fake_printer(
        reply: Option<Vec<u8>>,
    ) -> (PrinterEndpoint, tokio::task::JoinHandle<Vec<u8>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener binds");
        let port = listener.local_addr().expect("listener has an address").port();

        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("connection arrives");
            if let Some(reply) = reply {
                stream.write_all(&reply).await.expect("reply is writable");
                stream.flush().await.expect("reply flushes");
            }
            let mut received = Vec::new();
            stream
                .read_to_end(&mut received)
                .await
                .expect("payload is readable");
            received
        });

        (
            PrinterEndpoint {
                host: "127.0.0.1".into(),
                port,
            },
            handle,
        )
    }

    #[tokio::test]
    async fn send_job_writes_the_payload_verbatim() {
        let (endpoint, printer) = fake_printer(None).await;

        send_job(&endpoint, AddressPolicy::PERMISSIVE, b"raster bytes")
            .await
            .expect("job is sent");

        assert_eq!(printer.await.expect("printer task"), b"raster bytes");
    }

    #[tokio::test]
    async fn query_status_resets_the_printer_then_parses_the_reply() {
        let mut block = vec![0u8; STATUS_BLOCK_LEN];
        block[10] = 62;
        block[11] = 0x0B;
        block[17] = 29;
        let (endpoint, printer) = fake_printer(Some(block)).await;

        let status = query_status(&endpoint, AddressPolicy::PERMISSIVE)
            .await
            .expect("status is read");
        assert_eq!(status.media_kind.as_deref(), Some("die_cut"));
        assert_eq!(status.media_width_mm, 62);
        assert_eq!(status.media_length_mm, 29);

        let request = printer.await.expect("printer task");
        assert_eq!(&request[..INVALIDATE_BYTES], &vec![0u8; INVALIDATE_BYTES][..]);
        assert_eq!(&request[INVALIDATE_BYTES..], &[0x1B, b'@', 0x1B, b'i', b'S']);
    }

    #[tokio::test]
    async fn one_connection_carries_both_the_status_check_and_the_job() {
        let mut block = vec![0u8; STATUS_BLOCK_LEN];
        block[10] = 62;
        block[11] = 0x0B;
        block[17] = 29;
        let (endpoint, printer) = fake_printer(Some(block)).await;

        let mut connection = open(&endpoint, AddressPolicy::PERMISSIVE)
            .await
            .expect("connection opens");
        let status = connection.request_status().await.expect("status is read");
        assert_eq!(status.media_width_mm, 62);
        connection
            .write_job(b"raster bytes")
            .await
            .expect("job is sent");
        drop(connection);

        let received = printer.await.expect("printer task");
        assert_eq!(&received[received.len() - 12..], b"raster bytes");
    }
}
