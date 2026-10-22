/// Stylize variable names according to RM Basic's rules:
/// First char is uppercase and all others are lowercase, unless
/// preceeded by _.
/// 
/// For example:
///     mystring$ -> Mystring$
///     my_string$ -> My_String$
/// 
pub fn stylize(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    let mut capitalize_next = false;

    if let Some(first) = chars.next() {
        result.push(first.to_ascii_uppercase());
    }

    for c in chars {
        if c == '_' {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.push(c.to_ascii_uppercase());
            capitalize_next = false;
        } else if c.is_numeric() {
            result.push(c);
        } else {
            result.push(c.to_ascii_lowercase());
        }
    }

    result
}