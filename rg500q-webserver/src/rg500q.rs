pub const DEFAULT_SERIAL_PORT: &str = "/dev/ttyUSB3";
pub const QUECTEL_USB_VID: &str = "2c7c";

pub const LTE_ALL_BANDS: &str =
    "1:2:3:4:5:7:8:12:13:17:18:19:20:25:26:28:29:30:32:34:38:39:40:41:42:43:46:48:66:71";
pub const NR_ALL_BANDS: &str = "1:2:3:5:7:8:12:20:25:28:38:40:41:48:66:71:77:78:79";

pub fn lte_unlock_command() -> String {
    format!("AT+QNWPREFCFG=\"lte_band\",{}", LTE_ALL_BANDS)
}

pub fn nr_unlock_command() -> String {
    format!("AT+QNWPREFCFG=\"nr5g_band\",{}", NR_ALL_BANDS)
}

pub fn lte_cell_unlock_command() -> &'static str {
    "AT+QNWLOCK=\"common/lte\",0"
}

pub fn nr_cell_unlock_command() -> &'static str {
    "AT+QNWLOCK=\"common/5g\",0"
}

pub fn mode_auto_command() -> &'static str {
    "AT+QNWPREFCFG=\"mode_pref\",AUTO"
}

pub fn qnet_connect_command() -> &'static str {
    "AT+QNETDEVCTL=1,1,1"
}

pub fn qnet_disconnect_command() -> &'static str {
    "AT+QNETDEVCTL=1,1,0"
}

pub fn qnet_status_command() -> &'static str {
    "AT+CGPADDR"
}

pub fn band_csv_to_quectel_list(csv: &str) -> String {
    csv.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(":")
}

pub fn translate_legacy_command(command: &str) -> Option<String> {
    let clean = command.trim().trim_end_matches('\r').trim_end_matches('\n');
    let upper = clean.to_ascii_uppercase();

    match upper.as_str() {
        "AT^MONSC" | "AT^MONSC?" => return Some("AT+QENG=\"servingcell\"".to_string()),
        "AT^HCSQ?" | "AT^HCSQ" | "AT^CERSSI?" | "AT^CERSSI" => {
            return Some("AT+QCSQ".to_string())
        }
        "AT^NDISDUP=1,1" => return Some(qnet_connect_command().to_string()),
        "AT^NDISDUP=1,0" => return Some(qnet_disconnect_command().to_string()),
        "AT^SETAUTODIAL?" | "AT^NDISSTATQRY?" => return Some(qnet_status_command().to_string()),
        "AT^LTEFREQLOCK=0" => return Some(lte_unlock_command()),
        "AT^NRFREQLOCK=0" => return Some(nr_unlock_command()),
        _ => {}
    }

    if upper.starts_with("AT^LTEFREQLOCK=3,") {
        if let Some(bands) = last_quoted_segment(clean) {
            return Some(format!("AT+QNWPREFCFG=\"lte_band\",{}", band_csv_to_quectel_list(bands)));
        }
    }

    if upper.starts_with("AT^NRFREQLOCK=3,") {
        if let Some(bands) = last_quoted_segment(clean) {
            return Some(format!("AT+QNWPREFCFG=\"nr5g_band\",{}", band_csv_to_quectel_list(bands)));
        }
    }

    if upper.starts_with("AT^SYSCFGEX") {
        return Some(mode_auto_command().to_string());
    }

    None
}

fn last_quoted_segment(input: &str) -> Option<&str> {
    let end = input.rfind('"')?;
    let start = input[..end].rfind('"')?;
    Some(&input[start + 1..end])
}
