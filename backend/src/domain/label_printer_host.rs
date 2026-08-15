use std::net::IpAddr;
use std::ops::Deref;

/// Where a label printer can be reached: an IP literal or a DNS hostname.
///
/// The port lives in its own column, so anything that looks like it carries one
/// — or a scheme, a path, or credentials — is rejected rather than quietly
/// mangled at connect time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelPrinterHost(String);

impl LabelPrinterHost {
    pub fn parse(s: String) -> Result<Self, String> {
        let trimmed = s.trim();

        if trimmed.is_empty() {
            return Err("Printer host is required.".into());
        }
        if trimmed.len() > 253 {
            return Err(format!("{s} is not a valid printer host."));
        }

        // An IP literal is accepted as-is; this is also what lets IPv6
        // addresses through despite the colon check below.
        if trimmed.parse::<IpAddr>().is_ok() {
            return Ok(Self(trimmed.to_string()));
        }

        if !is_valid_hostname(trimmed) {
            return Err(format!("{s} is not a valid printer host."));
        }

        Ok(Self(trimmed.to_ascii_lowercase()))
    }
}

/// Whether every dot-separated label is a legal DNS label.
fn is_valid_hostname(host: &str) -> bool {
    let labels: Vec<&str> = host.split('.').collect();
    if labels.iter().any(|label| label.is_empty()) {
        return false;
    }

    labels.iter().all(|label| {
        label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

impl AsRef<str> for LabelPrinterHost {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for LabelPrinterHost {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<LabelPrinterHost> for String {
    fn from(host: LabelPrinterHost) -> Self {
        host.0
    }
}

#[cfg(test)]
mod tests {
    use super::LabelPrinterHost;
    use claims::{assert_err, assert_ok};

    #[test]
    fn ip_literals_are_accepted() {
        for host in ["192.168.1.50", "10.0.0.7", "::1", "fe80::1"] {
            assert_ok!(LabelPrinterHost::parse(host.into()));
        }
    }

    #[test]
    fn hostnames_are_accepted_and_normalised() {
        let host = LabelPrinterHost::parse("  Label-Printer.Lab.Local  ".into())
            .expect("hostname is valid");
        assert_eq!(host.as_ref(), "label-printer.lab.local");
    }

    #[test]
    fn a_bare_label_is_a_valid_hostname() {
        assert_ok!(LabelPrinterHost::parse("printer1".into()));
    }

    #[test]
    fn empty_hosts_are_rejected() {
        assert_err!(LabelPrinterHost::parse(String::new()));
        assert_err!(LabelPrinterHost::parse("   ".into()));
    }

    #[test]
    fn hosts_carrying_a_port_are_rejected() {
        // The port has its own column; accepting it here would silently drop it.
        assert_err!(LabelPrinterHost::parse("192.168.1.50:9100".into()));
        assert_err!(LabelPrinterHost::parse("printer.lab.local:9100".into()));
    }

    #[test]
    fn hosts_carrying_a_scheme_path_or_credentials_are_rejected() {
        for host in [
            "http://192.168.1.50",
            "192.168.1.50/print",
            "user@192.168.1.50",
            "printer lab",
        ] {
            assert_err!(LabelPrinterHost::parse(host.into()));
        }
    }

    #[test]
    fn malformed_hostnames_are_rejected() {
        for host in [
            "-printer",
            "printer-",
            "printer..lab",
            ".printer",
            "printer.",
        ] {
            assert_err!(LabelPrinterHost::parse(host.into()));
        }
    }

    #[test]
    fn overly_long_hosts_are_rejected() {
        assert_err!(LabelPrinterHost::parse("a".repeat(254)));
    }

    #[test]
    fn overly_long_labels_are_rejected() {
        assert_err!(LabelPrinterHost::parse(format!("{}.lab", "a".repeat(64))));
    }
}
