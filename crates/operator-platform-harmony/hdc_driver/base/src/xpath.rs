use serde_json::{Map, Value};

use crate::driver::Driver;
use crate::error::{HdcError, Result};
use crate::types::{Bounds, Point};

pub struct XPathNode<'a> {
    driver: &'a mut Driver,
    bounds: Option<Bounds>,
    info: Option<Map<String, Value>>,
}

impl<'a> XPathNode<'a> {
    pub(crate) fn find(driver: &'a mut Driver, expression: &str) -> Result<Self> {
        let hierarchy = driver.dump_hierarchy()?;
        let matcher = XPathMatcher::parse(expression)?;
        let matched = find_first_match(&hierarchy, &matcher);
        Ok(Self {
            driver,
            bounds: matched.as_ref().and_then(|item| item.bounds),
            info: matched.map(|item| item.info),
        })
    }

    pub fn exists(&self) -> bool {
        self.bounds.is_some()
    }

    pub fn text(&self) -> Result<Option<String>> {
        Ok(self
            .info
            .as_ref()
            .and_then(|info| info.get("text"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned))
    }

    pub fn bounds(&self) -> Result<Option<Bounds>> {
        Ok(self.bounds)
    }

    pub fn center(&self) -> Result<Option<Point>> {
        Ok(self.bounds.map(Bounds::center))
    }

    pub fn click(&mut self) -> Result<()> {
        let bounds = self.require_bounds()?;
        let center = bounds.center();
        self.driver.click(center.x, center.y)
    }

    pub fn click_if_exists(&mut self) -> Result<bool> {
        if !self.exists() {
            return Ok(false);
        }
        self.click()?;
        Ok(true)
    }

    pub fn double_click(&mut self) -> Result<()> {
        let bounds = self.require_bounds()?;
        let center = bounds.center();
        self.driver.double_click(center.x, center.y)
    }

    pub fn long_click(&mut self) -> Result<()> {
        let bounds = self.require_bounds()?;
        let center = bounds.center();
        self.driver.long_click(center.x, center.y)
    }

    pub fn input_text(&mut self, text: &str) -> Result<()> {
        self.click()?;
        self.driver.input_text(text)
    }

    fn require_bounds(&self) -> Result<Bounds> {
        self.bounds
            .ok_or_else(|| HdcError::protocol("xpath node not found"))
    }
}

#[derive(Debug)]
struct XPathMatcher {
    node_kind: Option<String>,
    predicate_groups: Vec<Vec<Predicate>>,
}

#[derive(Debug)]
struct XPathMatch {
    bounds: Option<Bounds>,
    info: Map<String, Value>,
}

impl XPathMatcher {
    fn parse(expression: &str) -> Result<Self> {
        let expression = expression.trim();
        if !expression.starts_with("//") {
            return Err(HdcError::protocol("xpath must start with //"));
        }
        let body = &expression[2..];
        let (node_kind, predicates) = if let Some(start) = body.find('[') {
            if !body.ends_with(']') {
                return Err(HdcError::protocol("invalid xpath expression"));
            }
            (&body[..start], Some(&body[start + 1..body.len() - 1]))
        } else {
            (body, None)
        };

        let node_kind = match node_kind.trim() {
            "" | "*" => None,
            value => Some(value.to_string()),
        };

        let predicate_groups = match predicates {
            Some(raw) => raw
                .split(" or ")
                .map(|group| {
                    group
                        .split(" and ")
                        .map(|item| Predicate::parse(item.trim()))
                        .collect::<Result<Vec<Predicate>>>()
                })
                .collect::<Result<Vec<Vec<Predicate>>>>()?,
            None => vec![Vec::new()],
        };
        Ok(Self {
            node_kind,
            predicate_groups,
        })
    }
}

fn find_first_match(node: &Value, matcher: &XPathMatcher) -> Option<XPathMatch> {
    let attributes = node.get("attributes")?.as_object()?;
    if matcher.matches(attributes) {
        return Some(XPathMatch {
            bounds: attributes
                .get("bounds")
                .and_then(Value::as_str)
                .and_then(parse_bounds_string),
            info: attributes.clone(),
        });
    }

    let children = node.get("children")?.as_array()?;
    for child in children {
        if let Some(matched) = find_first_match(child, matcher) {
            return Some(matched);
        }
    }
    None
}

impl XPathMatcher {
    fn matches(&self, attributes: &Map<String, Value>) -> bool {
        if let Some(kind) = &self.node_kind
            && attributes.get("type").and_then(Value::as_str) != Some(kind.as_str())
        {
            return false;
        }
        self.predicate_groups
            .iter()
            .any(|group| group.iter().all(|predicate| predicate.matches(attributes)))
    }
}

#[derive(Debug)]
enum Predicate {
    Equals { attribute: String, value: String },
    Contains { attribute: String, value: String },
    StartsWith { attribute: String, value: String },
}

impl Predicate {
    fn parse(input: &str) -> Result<Self> {
        if let Some(inner) = input
            .strip_prefix("contains(")
            .and_then(|value| value.strip_suffix(')'))
        {
            let (attribute, value) = split_call_args(inner)?;
            return Ok(Self::Contains {
                attribute: normalize_attribute(attribute),
                value: parse_quoted(value)?,
            });
        }
        if let Some(inner) = input
            .strip_prefix("starts-with(")
            .and_then(|value| value.strip_suffix(')'))
        {
            let (attribute, value) = split_call_args(inner)?;
            return Ok(Self::StartsWith {
                attribute: normalize_attribute(attribute),
                value: parse_quoted(value)?,
            });
        }
        let (attribute, value) = input
            .split_once('=')
            .ok_or_else(|| HdcError::protocol("invalid xpath predicate"))?;
        Ok(Self::Equals {
            attribute: normalize_attribute(attribute),
            value: parse_quoted(value)?,
        })
    }

    fn matches(&self, attributes: &Map<String, Value>) -> bool {
        let (attribute, expected, mode) = match self {
            Self::Equals { attribute, value } => (attribute, value, 0_u8),
            Self::Contains { attribute, value } => (attribute, value, 1_u8),
            Self::StartsWith { attribute, value } => (attribute, value, 2_u8),
        };
        let actual = attributes
            .get(attribute)
            .and_then(Value::as_str)
            .unwrap_or_default();
        match mode {
            0 => actual == expected,
            1 => actual.contains(expected),
            2 => actual.starts_with(expected),
            _ => false,
        }
    }
}

fn split_call_args(input: &str) -> Result<(&str, &str)> {
    input
        .split_once(',')
        .map(|(left, right)| (left.trim(), right.trim()))
        .ok_or_else(|| HdcError::protocol("invalid xpath function arguments"))
}

fn normalize_attribute(input: &str) -> String {
    let input = input.trim();
    if input == "text()" {
        "text".to_string()
    } else {
        input.trim_start_matches('@').to_string()
    }
}

fn parse_quoted(input: &str) -> Result<String> {
    let value = input.trim();
    if value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
    {
        Ok(value[1..value.len() - 1].to_string())
    } else {
        Err(HdcError::protocol("xpath value must be quoted"))
    }
}

fn parse_bounds_string(value: &str) -> Option<Bounds> {
    let numbers = value
        .replace("][", ",")
        .replace(['[', ']'], "")
        .split(',')
        .map(str::trim)
        .map(str::parse::<i32>)
        .collect::<std::result::Result<Vec<i32>, _>>()
        .ok()?;
    if numbers.len() != 4 {
        return None;
    }
    Some(Bounds {
        left: numbers[0],
        top: numbers[1],
        right: numbers[2],
        bottom: numbers[3],
    })
}

#[cfg(test)]
mod tests {
    use super::{Predicate, XPathMatcher, find_first_match, parse_bounds_string};
    use serde_json::json;

    #[test]
    fn parse_simple_xpath_attribute_equals() {
        let matcher = XPathMatcher::parse("//*[@text='Hello']").unwrap();
        assert!(matcher.node_kind.is_none());
        assert_eq!(matcher.predicate_groups.len(), 1);
        assert_eq!(matcher.predicate_groups[0].len(), 1);
    }

    #[test]
    fn parse_xpath_with_type_and_multiple_predicates() {
        let matcher =
            XPathMatcher::parse("//Button[@text='Hello' and contains(@id,'primary')]").unwrap();
        assert_eq!(matcher.node_kind.as_deref(), Some("Button"));
        assert_eq!(matcher.predicate_groups.len(), 1);
        assert_eq!(matcher.predicate_groups[0].len(), 2);
    }

    #[test]
    fn parse_xpath_with_or_groups() {
        let matcher = XPathMatcher::parse("//*[@text='Hello' or @text='World']").unwrap();
        assert_eq!(matcher.predicate_groups.len(), 2);
    }

    #[test]
    fn parse_bounds_string_reads_four_edges() {
        let bounds = parse_bounds_string("[1,2][3,4]").unwrap();
        assert_eq!(bounds.left, 1);
        assert_eq!(bounds.top, 2);
        assert_eq!(bounds.right, 3);
        assert_eq!(bounds.bottom, 4);
    }

    #[test]
    fn find_first_match_recurses_children() {
        let tree = json!({
            "attributes": {"text": "", "type": "root", "bounds": "[0,0][10,10]"},
            "children": [
                {
                    "attributes": {"text": "Hello", "type": "Button", "id": "primary-1", "bounds": "[10,20][30,40]"},
                    "children": []
                }
            ]
        });
        let matcher =
            XPathMatcher::parse("//Button[@text='Hello' and starts-with(@id,'primary')]").unwrap();
        let matched = find_first_match(&tree, &matcher).unwrap();
        assert_eq!(
            matched.info.get("text").and_then(|v| v.as_str()),
            Some("Hello")
        );
        assert_eq!(matched.bounds.unwrap().left, 10);
    }

    #[test]
    fn predicate_matches_text_function_alias() {
        let attributes = json!({"text": "Hello World"}).as_object().unwrap().clone();
        let predicate = Predicate::parse("contains(text(), 'Hello')").unwrap();
        assert!(predicate.matches(&attributes));
    }
}
