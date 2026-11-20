//! .NET number formats (faithful copy of the `clr` oracle).
fn fmt_float_inv(x: f64, digits: usize) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity".into() } else { "-Infinity".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let s = format!("{:.*e}", digits - 1, x);
    let (m, e) = s.split_once('e').unwrap();
    let exp: i32 = e.parse().unwrap();
    let neg = m.starts_with('-');
    let m = m.trim_start_matches('-');
    let mut digs: String = m.chars().filter(|c| *c != '.').collect();
    while digs.len() > 1 && digs.ends_with('0') {
        digs.pop();
    }
    let sign = if neg { "-" } else { "" };
    if exp >= -5 && exp < digits as i32 {
        if exp >= 0 {
            let e = exp as usize;
            if digs.len() <= e + 1 {
                format!("{}{}{}", sign, digs, "0".repeat(e + 1 - digs.len()))
            } else {
                format!("{}{}.{}", sign, &digs[..e + 1], &digs[e + 1..])
            }
        } else {
            format!("{}0.{}{}", sign, "0".repeat((-exp - 1) as usize), digs)
        }
    } else {
        let mant = if digs.len() > 1 { format!("{}.{}", &digs[..1], &digs[1..]) } else { digs };
        format!("{}{}E{}{:02}", sign, mant, if exp < 0 { "-" } else { "+" }, exp.abs())
    }
}

fn group3(int: &str) -> String {
    let b = int.as_bytes();
    let mut out = String::new();
    for (i, c) in b.iter().enumerate() {
        if i > 0 && (b.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*c as char);
    }
    out
}

/// .NET number format (standard and custom).
fn format_number_inv(x: f64, is_int: bool, fmt: &str) -> String {
    let f = fmt.trim();
    let first = f.chars().next().unwrap_or('G');
    let rest = &f[first.len_utf8().min(f.len())..];
    if f.is_empty() {
        return if is_int { format!("{}", x as i64) } else { fmt_float_inv(x, 15) };
    }
    if first.is_ascii_alphabetic() && rest.chars().all(|c| c.is_ascii_digit()) {
        let prec: Option<usize> = if rest.is_empty() { None } else { rest.parse().ok() };
        match first.to_ascii_uppercase() {
            'X' => {
                let s = format!("{:X}", x as i64);
                return if let Some(p) = prec { format!("{:0>1$}", s, p) } else { s };
            }
            'D' => {
                let neg = x < 0.0;
                let s = format!("{}", (x as i64).abs());
                let s = if let Some(p) = prec { format!("{:0>1$}", s, p) } else { s };
                return if neg { format!("-{}", s) } else { s };
            }
            'F' => return format!("{:.*}", prec.unwrap_or(2), x),
            'N' => {
                let s = format!("{:.*}", prec.unwrap_or(2), x.abs());
                let (i, fr) = s.split_once('.').map(|(a, b)| (a.to_string(), Some(b.to_string()))).unwrap_or((s, None));
                let body = match fr { Some(fr) => format!("{}.{}", group3(&i), fr), None => group3(&i) };
                return if x < 0.0 { format!("-{}", body) } else { body };
            }
            'E' => {
                let p = prec.unwrap_or(6);
                let s = format!("{:.*e}", p, x);
                let (m, e) = s.split_once('e').unwrap();
                let ev: i32 = e.parse().unwrap();
                return format!("{}E{}{:03}", m, if ev < 0 { '-' } else { '+' }, ev.abs());
            }
            'G' => return if is_int { format!("{}", x as i64) } else { fmt_float_inv(x, prec.unwrap_or(15)) },
            _ => {}
        }
    }
    // custom format
    let (int_pat, frac_pat) = match f.split_once('.') {
        Some((a, b)) => (a, b),
        None => (f, ""),
    };
    let grouping = int_pat.contains(',');
    let int_zeros = int_pat.chars().filter(|c| *c == '0').count();
    let frac_max = frac_pat.chars().filter(|c| *c == '0' || *c == '#').count();
    let frac_min = frac_pat.chars().filter(|c| *c == '0').count();
    let neg = x < 0.0 && (x.abs() * 10f64.powi(frac_max as i32)).round() != 0.0;
    let s = format!("{:.*}", frac_max, x.abs());
    let (ip, fp) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), b.to_string()),
        None => (s.clone(), String::new()),
    };
    let mut ip = ip.trim_start_matches('0').to_string();
    while ip.len() < int_zeros {
        ip.insert(0, '0');
    }
    let ip = if grouping { group3(&ip) } else { ip };
    let mut fp = fp;
    while fp.len() > frac_min && fp.ends_with('0') {
        fp.pop();
    }
    let mut out = String::new();
    if neg {
        out.push('-');
    }
    out.push_str(&ip);
    if !fp.is_empty() {
        out.push('.');
        out.push_str(&fp);
    }
    if out.is_empty() || out == "-" {
        out.push('0');
    }
    out
}


// ------------------------------------------------------------------ number display culture
// Like Mono in the original game: separators of the system language (Windows in French: `50 000`, `48,9`).
// NGU_CULTURE=fr|en|de|invariant forces the culture (tests). ponytail: separator table per language (no CLDR data);
// identical copy in clr/src/bcl_core.rs and crates/ngu-rt/src/fmt.rs (oracle and native must stay equal).
pub fn culture() -> (char, char) {
    static C: std::sync::OnceLock<(char, char)> = std::sync::OnceLock::new();
    *C.get_or_init(|| {
        let l = std::env::var("NGU_CULTURE").ok().or_else(sys_lang).unwrap_or_default().to_ascii_lowercase();
        match &l[..l.len().min(2)] {
            "fr" | "ru" | "pl" | "cs" | "sk" | "sv" | "fi" | "nb" | "no" | "uk" | "hu" | "bg" => ('\u{a0}', ','),
            "de" | "es" | "it" | "pt" | "nl" | "tr" | "id" | "da" | "el" | "ro" | "hr" | "sl" | "sr" | "vi" => ('.', ','),
            _ => (',', '.'),
        }
    })
}
#[cfg(windows)]
fn sys_lang() -> Option<String> {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetUserDefaultLocaleName(buf: *mut u16, n: i32) -> i32;
    }
    let mut b = [0u16; 85];
    let n = unsafe { GetUserDefaultLocaleName(b.as_mut_ptr(), 85) };
    (n > 1).then(|| String::from_utf16_lossy(&b[..n as usize - 1]))
}
#[cfg(not(windows))]
fn sys_lang() -> Option<String> {
    ["LC_ALL", "LC_NUMERIC", "LANG"].iter().find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
}
/// Invariant text (`1,234.5`) -> display culture.
pub fn loc(s: String) -> String {
    let (g, d) = culture();
    if (g, d) == (',', '.') {
        return s;
    }
    s.chars().map(|c| match c { ',' => g, '.' => d, c => c }).collect()
}
/// Text typed in the display culture -> invariant text for parsing (Double.Parse).
pub fn deloc(s: &str) -> String {
    let (g, d) = culture();
    if d == '.' {
        return s.to_string();
    }
    s.chars().filter(|c| *c != g && *c != ' ').map(|c| if c == d { '.' } else { c }).collect()
}
pub fn fmt_float(x: f64, digits: usize) -> String {
    loc(fmt_float_inv(x, digits))
}
/// .NET number format (standard and custom), in the display culture.
pub fn format_number(x: f64, is_int: bool, fmt: &str) -> String {
    loc(format_number_inv(x, is_int, fmt))
}
