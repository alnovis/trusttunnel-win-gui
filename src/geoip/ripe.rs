//! Download and parse RIR delegated-extended statistics into a country CIDR
//! list. This is the direct reuse of the Keenetic S97geoip pipeline:
//!   download -> filter (cc, ipv4, skip summary) -> count_to_prefix.
//!
//! Pure Rust and network-only; testable off-Windows.

use super::cidr::count_to_prefix;

/// Map an RIR id to its delegated-extended-latest URL.
pub fn rir_url(rir: &str) -> Option<String> {
    let host = match rir {
        "ripencc" => "ftp.ripe.net/pub/stats/ripencc/delegated-ripencc-extended-latest",
        "arin" => "ftp.arin.net/pub/stats/arin/delegated-arin-extended-latest",
        "apnic" => "ftp.apnic.net/pub/stats/apnic/delegated-apnic-extended-latest",
        "lacnic" => "ftp.lacnic.net/pub/stats/lacnic/delegated-lacnic-extended-latest",
        "afrinic" => "ftp.afrinic.net/pub/stats/afrinic/delegated-afrinic-extended-latest",
        _ => return None,
    };
    Some(format!("https://{host}"))
}

/// Parse the raw delegated-extended body, returning CIDR strings ("1.2.3.0/24")
/// for the given ISO country code.
///
/// Line format is pipe-delimited:
///   registry|cc|type|start|value|date|status[|extensions]
/// We keep rows where field[1]==cc, field[2]=="ipv4", field[3] != "*".
/// field[3] is the start IP, field[4] the address count.
pub fn parse_country_cidrs(body: &str, country: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in body.lines() {
        if line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split('|').collect();
        if f.len() < 5 {
            continue;
        }
        if f[1] != country || f[2] != "ipv4" || f[3] == "*" {
            continue;
        }
        let count: u64 = match f[4].parse() {
            Ok(n) if n > 0 => n,
            _ => continue,
        };
        let prefix = count_to_prefix(count);
        out.push(format!("{}/{}", f[3], prefix));
    }
    out
}

/// Download the delegated file over HTTPS.
///
/// IMPORTANT: on a country where RIPE is blocked this only works when the VPN
/// is already up (full/general tunnel routes the foreign RIPE IP through the
/// endpoint). The caller decides when to invoke this -- see `geoip::refresh`.
pub fn download(url: &str) -> Result<String, String> {
    let resp = ureq::get(url)
        .timeout(std::time::Duration::from_secs(120))
        .call()
        .map_err(|e| format!("download failed: {e}"))?;
    resp.into_string()
        .map_err(|e| format!("read body failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filters_and_converts() {
        let body = "\
2.3|ripencc|*|*|*|*|summary\n\
ripencc|RU|ipv4|1.2.3.0|256|20200101|allocated\n\
ripencc|RU|ipv6|2001:db8::|32|20200101|allocated\n\
ripencc|DE|ipv4|9.9.9.0|256|20200101|allocated\n\
ripencc|RU|ipv4|*|0|20200101|reserved\n\
ripencc|RU|ipv4|5.6.0.0|65536|20200101|allocated";
        let cidrs = parse_country_cidrs(body, "RU");
        assert_eq!(cidrs, vec!["1.2.3.0/24".to_string(), "5.6.0.0/16".to_string()]);
    }
}
