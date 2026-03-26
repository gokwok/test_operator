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
    attribute: String,
    value: String,
}

#[derive(Debug)]
struct XPathMatch {
    bounds: Option<Bounds>,
    info: Map<String, Value>,
}

impl XPathMatcher {
    fn parse(expression: &str) -> Result<Self> {
        let expression = expression.trim();
        let start = expression
            .find("[@")
            .ok_or_else(|| HdcError::protocol("only simple attribute xpath is supported"))?;
        if !expression.ends_with(']') {
            return Err(HdcError::protocol("invalid xpath expression"));
        }
        let predicate = &expression[start + 2..expression.len() - 1];
        let (attribute, raw_value) = predicate
            .split_once('=')
            .ok_or_else(|| HdcError::protocol("invalid xpath predicate"))?;
        let value = raw_value.trim();
        let value = if value.len() >= 2
            && ((value.starts_with('\'') && value.ends_with('\''))
                || (value.starts_with('"') && value.ends_with('"')))
        {
            value[1..value.len() - 1].to_string()
        } else {
            return Err(HdcError::protocol("xpath value must be quoted"));
        };
        Ok(Self {
            attribute: attribute.trim().to_string(),
            value,
        })
    }
}

fn find_first_match(node: &Value, matcher: &XPathMatcher) -> Option<XPathMatch> {
    let attributes = node.get("attributes")?.as_object()?;
    if attributes
        .get(&matcher.attribute)
        .and_then(Value::as_str)
        == Some(matcher.value.as_str())
    {
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
    use super::{XPathMatcher, find_first_match, parse_bounds_string};
    use serde_json::json;

    #[test]
    fn parse_simple_xpath_attribute_equals() {
        let matcher = XPathMatcher::parse("//*[@text='Hello']").unwrap();
        assert_eq!(matcher.attribute, "text");
        assert_eq!(matcher.value, "Hello");
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
            "attributes": {"text": "", "bounds": "[0,0][10,10]"},
            "children": [
                {
                    "attributes": {"text": "Hello", "bounds": "[10,20][30,40]"},
                    "children": []
                }
            ]
        });
        let matcher = XPathMatcher::parse("//*[@text='Hello']").unwrap();
        let matched = find_first_match(&tree, &matcher).unwrap();
        assert_eq!(matched.info.get("text").and_then(|v| v.as_str()), Some("Hello"));
        assert_eq!(matched.bounds.unwrap().left, 10);
    }
}
