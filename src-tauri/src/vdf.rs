use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub enum Value {
    Text(String),
    Object(BTreeMap<String, Value>),
}
impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(m) => m.get(key),
            _ => None,
        }
    }
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s),
            _ => None,
        }
    }
    pub fn object(&self) -> Option<&BTreeMap<String, Value>> {
        match self {
            Self::Object(m) => Some(m),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_whitespace() || c == '\u{feff}' {
            continue;
        }
        if c == '/' && chars.peek() == Some(&'/') {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    break;
                }
            }
            continue;
        }
        if c == '{' || c == '}' {
            tokens.push(c.to_string());
            continue;
        }
        if c == '"' {
            let mut s = String::new();
            let mut ended = false;
            while let Some(ch) = chars.next() {
                if ch == '"' {
                    ended = true;
                    break;
                }
                if ch == '\\' {
                    match chars.next() {
                        Some('\\') => s.push('\\'),
                        Some('"') => s.push('"'),
                        Some(other) => {
                            s.push('\\');
                            s.push(other);
                        }
                        None => return Err("Unfinished escape".into()),
                    }
                } else {
                    s.push(ch);
                }
            }
            if !ended {
                return Err("Unclosed string".into());
            }
            tokens.push(format!("s{s}"));
        } else {
            let mut s = String::from(c);
            while chars
                .peek()
                .is_some_and(|x| !x.is_whitespace() && *x != '{' && *x != '}')
            {
                s.push(chars.next().unwrap());
            }
            tokens.push(format!("s{s}"));
        }
    }
    fn object(
        tokens: &[String],
        pos: &mut usize,
        nested: bool,
        depth: usize,
    ) -> Result<Value, String> {
        if depth > 32 {
            return Err("VDF nesting limit".into());
        }
        let mut map = BTreeMap::new();
        while *pos < tokens.len() {
            if tokens[*pos] == "}" {
                if !nested {
                    return Err("Unexpected closing brace".into());
                }
                *pos += 1;
                return Ok(Value::Object(map));
            }
            let key = tokens[*pos]
                .strip_prefix('s')
                .ok_or("Expected key")?
                .to_string();
            *pos += 1;
            let value = match tokens.get(*pos).map(String::as_str) {
                Some("{") => {
                    *pos += 1;
                    object(tokens, pos, true, depth + 1)?
                }
                Some(s) if s.starts_with('s') => {
                    *pos += 1;
                    Value::Text(s[1..].to_string())
                }
                _ => return Err("Expected value".into()),
            };
            map.insert(key, value);
        }
        if nested {
            Err("Unclosed object".into())
        } else {
            Ok(Value::Object(map))
        }
    }
    object(&tokens, &mut 0, false, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn steam_paths_and_nested_objects() {
        let v =
            parse(r#""libraryfolders" { "0" { "path" "D:\\SteamLibrary" "apps" { "123" "1" } } }"#)
                .unwrap();
        assert_eq!(
            v.get("libraryfolders")
                .unwrap()
                .get("0")
                .unwrap()
                .get("path")
                .unwrap()
                .text(),
            Some("D:\\SteamLibrary")
        );
    }
    #[test]
    fn malformed_input_is_rejected() {
        assert!(parse("\"AppState\" { \"name\" \"broken").is_err());
    }
    #[test]
    fn comments_are_ignored() {
        let v = parse("// hello\n\"name\" \"Game // title\"").unwrap();
        assert_eq!(v.get("name").unwrap().text(), Some("Game // title"));
    }
}
