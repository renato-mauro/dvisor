pub fn suggest(names: &Vec<String>, name: &String) -> Option<String> {
    use strsim::jaro_winkler;
    names
        .iter()
        .map(|s| (s,jaro_winkler(name, s)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
        .and_then(|(cand, score)| if score >= 0.85 { Some(cand.clone()) } else { None })
}

pub fn parens_balanced(s: &str) -> bool {
    let mut depth: i32 = 0;
    for ch in s.chars() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth < 0 { return false; }
            }
            _ => {}
        }
    }
    depth == 0
}
