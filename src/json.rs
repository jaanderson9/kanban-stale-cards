// Minimal JSON reader. We only need enough to walk a Trello-style export
// (objects, arrays, strings, numbers, bools, null) and pull fields out of it,
// so this skips anything a general-purpose parser would need for round-tripping.

#[derive(Debug, Clone)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&Vec<Value>> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

pub fn parse(input: &str) -> Result<Value, String> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_ws();
    if parser.pos != parser.chars.len() {
        return Err(format!("unexpected trailing data at position {}", parser.pos));
    }
    Ok(value)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser {
            chars: input.chars().collect(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(c) if c.is_whitespace()) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        match self.peek() {
            Some('{') => self.parse_object(),
            Some('[') => self.parse_array(),
            Some('"') => self.parse_string().map(Value::String),
            Some('t') => self.parse_keyword("true", Value::Bool(true)),
            Some('f') => self.parse_keyword("false", Value::Bool(false)),
            Some('n') => self.parse_keyword("null", Value::Null),
            Some(c) if c == '-' || c.is_ascii_digit() => self.parse_number(),
            Some(c) => Err(format!("unexpected character '{}' at position {}", c, self.pos)),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_keyword(&mut self, word: &str, value: Value) -> Result<Value, String> {
        for expected in word.chars() {
            match self.advance() {
                Some(c) if c == expected => {}
                _ => return Err(format!("invalid literal near position {}", self.pos)),
            }
        }
        Ok(value)
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.advance(); // opening quote
        let mut out = String::new();
        loop {
            match self.advance() {
                None => return Err("unterminated string".to_string()),
                Some('"') => return Ok(out),
                Some('\\') => match self.advance() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('/') => out.push('/'),
                    Some('b') => out.push('\u{0008}'),
                    Some('f') => out.push('\u{000C}'),
                    Some('n') => out.push('\n'),
                    Some('r') => out.push('\r'),
                    Some('t') => out.push('\t'),
                    Some('u') => {
                        let cp = self.read_hex4()?;
                        if (0xD800..=0xDBFF).contains(&cp) {
                            if self.advance() != Some('\\') || self.advance() != Some('u') {
                                return Err("expected low surrogate after high surrogate".to_string());
                            }
                            let low = self.read_hex4()?;
                            let combined = 0x10000 + (((cp - 0xD800) as u32) << 10) + (low - 0xDC00) as u32;
                            if let Some(ch) = char::from_u32(combined) {
                                out.push(ch);
                            }
                        } else if let Some(ch) = char::from_u32(cp as u32) {
                            out.push(ch);
                        }
                    }
                    _ => return Err("invalid escape sequence in string".to_string()),
                },
                Some(c) => out.push(c),
            }
        }
    }

    fn read_hex4(&mut self) -> Result<u16, String> {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let c = self.advance().ok_or("unexpected end of input in \\u escape")?;
            let digit = c.to_digit(16).ok_or("invalid hex digit in \\u escape")?;
            value = value * 16 + digit as u16;
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some('-') {
            self.advance();
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.advance();
        }
        if self.peek() == Some('.') {
            self.advance();
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            self.advance();
            if matches!(self.peek(), Some('+') | Some('-')) {
                self.advance();
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.advance();
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        text.parse::<f64>()
            .map(Value::Number)
            .map_err(|e| format!("invalid number '{}': {}", text, e))
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.advance(); // [
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(']') {
            self.advance();
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.parse_value()?);
            self.skip_ws();
            match self.advance() {
                Some(',') => self.skip_ws(),
                Some(']') => return Ok(Value::Array(items)),
                _ => return Err("expected ',' or ']' in array".to_string()),
            }
        }
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        self.advance(); // {
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some('}') {
            self.advance();
            return Ok(Value::Object(pairs));
        }
        loop {
            self.skip_ws();
            if self.peek() != Some('"') {
                return Err("expected string key in object".to_string());
            }
            let key = self.parse_string()?;
            self.skip_ws();
            if self.advance() != Some(':') {
                return Err("expected ':' after object key".to_string());
            }
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_ws();
            match self.advance() {
                Some(',') => self.skip_ws(),
                Some('}') => return Ok(Value::Object(pairs)),
                _ => return Err("expected ',' or '}' in object".to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_primitives() {
        assert!(matches!(parse("null").unwrap(), Value::Null));
        assert!(matches!(parse("true").unwrap(), Value::Bool(true)));
        assert!(matches!(parse("false").unwrap(), Value::Bool(false)));
        match parse("\"hi\"").unwrap() {
            Value::String(s) => assert_eq!(s, "hi"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn parses_numbers() {
        let cases = [
            ("0", 0.0),
            ("-12", -12.0),
            ("3.5", 3.5),
            ("-0.25", -0.25),
            ("1e3", 1000.0),
            ("2.5e-2", 0.025),
        ];
        for (input, expected) in cases {
            match parse(input).unwrap() {
                Value::Number(n) => assert_eq!(n, expected, "input {}", input),
                other => panic!("expected number for '{}', got {:?}", input, other),
            }
        }
    }

    #[test]
    fn parses_nested_object_and_array() {
        let input = r#"{"a": 1, "b": [1, 2, {"c": "d"}], "e": null}"#;
        let root = parse(input).unwrap();
        assert!(matches!(root.get("a"), Some(Value::Number(n)) if *n == 1.0));
        let arr = root.get("b").and_then(Value::as_array).unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[2].get("c").and_then(Value::as_str), Some("d"));
        assert!(matches!(root.get("e"), Some(Value::Null)));
        assert!(root.get("missing").is_none());
    }

    #[test]
    fn parses_string_escapes() {
        let input = r#""line1\nline2\t\"quoted\"\\end""#;
        match parse(input).unwrap() {
            Value::String(s) => assert_eq!(s, "line1\nline2\t\"quoted\"\\end"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn parses_unicode_escape() {
        let input = "\"\\u00e9\""; // e with acute accent
        match parse(input).unwrap() {
            Value::String(s) => assert_eq!(s, "\u{00e9}"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn parses_surrogate_pair_escape() {
        // high/low surrogate pair for the grinning face emoji, U+1F600
        let input = "\"\\ud83d\\ude00\"";
        match parse(input).unwrap() {
            Value::String(s) => assert_eq!(s, "\u{1F600}"),
            other => panic!("expected string, got {:?}", other),
        }
    }

    #[test]
    fn as_helpers_reject_wrong_variant() {
        let s = Value::String("x".to_string());
        assert_eq!(s.as_str(), Some("x"));
        assert!(s.as_array().is_none());
        assert!(s.as_bool().is_none());
    }

    #[test]
    fn rejects_trailing_garbage() {
        assert!(parse("{}garbage").is_err());
    }

    #[test]
    fn rejects_unterminated_string() {
        assert!(parse("\"abc").is_err());
    }

    #[test]
    fn rejects_bad_literal() {
        assert!(parse("nul").is_err());
    }

    #[test]
    fn rejects_missing_comma_in_array() {
        assert!(parse("[1 2]").is_err());
    }

    #[test]
    fn rejects_missing_colon_in_object() {
        assert!(parse(r#"{"a" 1}"#).is_err());
    }

    #[test]
    fn empty_array_and_object() {
        assert!(matches!(parse("[]").unwrap(), Value::Array(v) if v.is_empty()));
        assert!(matches!(parse("{}").unwrap(), Value::Object(v) if v.is_empty()));
    }
}
