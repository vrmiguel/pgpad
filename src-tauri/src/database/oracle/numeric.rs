pub fn normalize_number_string(s: &str) -> String {
    let mut out = s.to_string();
    if let Some(dot) = out.find('.') {
        let (int_part, frac_part) = out.split_at(dot + 1);
        let mut frac = frac_part.to_string();
        while frac.ends_with('0') { frac.pop(); }
        if frac.is_empty() { out = int_part[..int_part.len()-1].to_string(); }
        else { out = format!("{}{}", int_part, frac); }
    }
    out
}

pub fn pad_number_scale(s: &str, scale: usize) -> String {
    if scale == 0 { return s.to_string(); }
    let mut out = s.to_string();
    if let Some(dot) = out.find('.') {
        let frac_len = out.len() - dot - 1;
        if frac_len < scale {
            out.push_str(&"0".repeat(scale - frac_len));
        }
    } else {
        out.push('.');
        out.push_str(&"0".repeat(scale));
    }
    out
}
