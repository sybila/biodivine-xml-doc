// TODO: calculate the minimum number of methods needed for manipulating tree
// and other helper methods should depend on that
// even if the performance takes a little hit.

mod element;
mod error;

pub use crate::element::Element;
pub use crate::error::{Error, Result};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::HashMap;
use std::io::{BufRead, Write};

#[cfg(debug_assertions)]
macro_rules! debug {
    ($x:expr) => {
        println!("{:?}", $x)
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadOptions {
    pub empty_text_node: bool, // <tag></tag> will have a Node::Text("") as its children, while <tag /> won't.
}

impl ReadOptions {
    pub fn default() -> ReadOptions {
        ReadOptions {
            empty_text_node: true,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Node {
    Element(Element),
    Text(String),
    Comment(String),
    CData(String),
    Decl {
        version: String,
        encoding: Option<String>,
        standalone: Option<String>,
    },
    PI(String),
    DocType(String),
}

impl Node {
    pub fn as_element(&self) -> Option<Element> {
        match self {
            Self::Element(elem) => Some(*elem),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ElementData {
    raw_name: String,
    attributes: HashMap<String, String>, // q:attr="val" => {"q:attr": "val"}
    namespace_decls: HashMap<String, String>, // local namespace newly defined in attributes
    parent: Option<Element>,
    children: Vec<Node>,
}

impl ElementData {}

#[derive(Debug, PartialEq, Eq)]
pub struct Document {
    pub read_opts: ReadOptions,
    counter: usize, // == self.store.len()
    store: Vec<ElementData>,
    nodes: Vec<Node>,
}

impl Document {
    pub fn new() -> Document {
        Document {
            read_opts: ReadOptions::default(),
            counter: 0,
            store: Vec::new(),
            nodes: Vec::new(),
        }
    }

    pub fn nodes(&self) -> &Vec<Node> {
        &self.nodes
    }

    pub fn remove_node(&mut self, index: usize) -> Node {
        self.nodes.remove(index)
    }

    pub fn push_node(&mut self, node: Node) -> Result<()> {
        if let Node::Element(element) = node {
            if element.has_parent(self) {
                return Err(Error::HasAParent);
            }
            element.detatch_from_parent(self);
        }
        self.nodes.push(node);
        Ok(())
    }

    pub fn insert_node(&mut self, index: usize, node: Node) -> Result<()> {
        if let Node::Element(element) = node {
            if element.has_parent(self) {
                return Err(Error::HasAParent);
            }
            element.detatch_from_parent(self);
        }
        self.nodes.insert(index, node);
        Ok(())
    }
}

// Read and write
impl Document {
    pub fn from_str(str: &str) -> Result<Document> {
        let mut document = Document::new();
        document.read_str(str)?;
        Ok(document)
    }

    pub fn from_reader<R: BufRead>(reader: R) -> Result<Document> {
        let mut document = Document::new();
        document.read_reader(reader)?;
        Ok(document)
    }

    pub fn read_str(&mut self, str: &str) -> Result<()> {
        if !self.store.is_empty() {
            return Err(Error::NotEmpty);
        }
        let reader = Reader::from_str(str);
        self.read(reader)?;
        Ok(())
    }

    pub fn read_reader<R: BufRead>(&mut self, reader: R) -> Result<()> {
        if !self.store.is_empty() {
            return Err(Error::NotEmpty);
        }
        let reader = Reader::from_reader(reader);
        self.read(reader)?;
        Ok(())
    }

    fn handle_bytes_start<B: BufRead>(
        &mut self,
        reader: &Reader<B>,
        element_stack: &Vec<Element>,
        ev: &BytesStart,
    ) -> Result<Element> {
        let raw_name = reader.decode(ev.name()).to_string();
        let element = Element::new(self, raw_name);
        let mut namespaces = HashMap::new();
        let attributes = element.mut_attributes(self);
        for attr in ev.attributes() {
            let attr = attr?;
            let key = reader.decode(attr.key).to_string();
            let value = attr.unescape_and_decode_value(reader)?;
            if key == "xmlns" {
                namespaces.insert(String::new(), value);
                continue;
            } else if let Some(prefix) = key.strip_prefix("xmlns:") {
                namespaces.insert(prefix.to_owned(), value);
                continue;
            }
            attributes.insert(key, value);
        }
        element.mut_namespace_declarations(self).extend(namespaces);
        let node = Node::Element(element);
        self.handle_push_node(element_stack, node);
        Ok(element)
    }

    fn handle_push_node(&mut self, element_stack: &Vec<Element>, node: Node) {
        match element_stack.last() {
            Some(parent) => parent.push_child(self, node).unwrap(),
            None => self.nodes.push(node),
        }
    }

    fn read<B: BufRead>(&mut self, mut reader: Reader<B>) -> Result<()> {
        reader.trim_text(true);

        let mut buf = Vec::new();
        let mut element_stack: Vec<Element> = vec![]; // root element in element_stack
        loop {
            let ev = reader.read_event(&mut buf);
            #[cfg(debug_assertions)]
            debug!(ev);
            match ev {
                Ok(Event::Start(ref ev)) => {
                    let element = self.handle_bytes_start(&reader, &element_stack, ev)?;
                    element_stack.push(element);
                }
                Ok(Event::End(_)) => {
                    let last_elem = element_stack.pop();
                    // distinguish <tag></tag> and <tag />
                    if self.read_opts.empty_text_node {
                        if let Some(elem) = last_elem {
                            if !elem.has_children(self) {
                                elem.push_child(self, Node::Text(String::new())).unwrap();
                            }
                        }
                    }
                }
                Ok(Event::Empty(ref ev)) => {
                    self.handle_bytes_start(&reader, &element_stack, ev)?;
                }
                Ok(Event::Text(ev)) => {
                    let node = Node::Text(ev.unescape_and_decode(&reader)?);
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::Comment(ev)) => {
                    let node = Node::Comment(ev.unescape_and_decode(&reader)?);
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::CData(ev)) => {
                    let node = Node::CData(ev.unescape_and_decode(&reader)?);
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::PI(ev)) => {
                    let node = Node::PI(ev.unescape_and_decode(&reader)?);
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::DocType(ev)) => {
                    let node = Node::DocType(ev.unescape_and_decode(&reader)?);
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::Decl(ev)) => {
                    let version = String::from_utf8_lossy(&ev.version()?).into_owned();
                    let encoding = match ev.encoding() {
                        Some(res) => Some(String::from_utf8_lossy(&res?).into_owned()),
                        None => None,
                    };
                    let standalone = match ev.standalone() {
                        Some(res) => Some(String::from_utf8_lossy(&res?).into_owned()),
                        None => None,
                    };
                    let node = Node::Decl {
                        version,
                        encoding,
                        standalone,
                    };
                    self.handle_push_node(&element_stack, node);
                }
                Ok(Event::Eof) => return Ok(()),
                Err(e) => return Err(Error::from(e)),
            }
        }
    }

    pub fn write_str(&self) -> Result<String> {
        let mut buf: Vec<u8> = Vec::new();
        self.write(&mut buf)?;
        Ok(String::from_utf8(buf).unwrap())
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        let mut writer = Writer::new_with_indent(writer, b' ', 4);
        self.write_nodes(&mut writer, &self.nodes)?;
        writer.write_event(Event::Eof)?;
        Ok(())
    }

    fn write_nodes(&self, writer: &mut Writer<impl Write>, nodes: &[Node]) -> Result<()> {
        for node in nodes {
            match node {
                Node::Element(eid) => self.write_element(writer, *eid)?,
                Node::Text(text) => {
                    writer.write_event(Event::Text(BytesText::from_escaped_str(text)))?
                }
                Node::CData(text) => {
                    writer.write_event(Event::CData(BytesText::from_escaped_str(text)))?
                }
                Node::Comment(text) => {
                    writer.write_event(Event::Comment(BytesText::from_escaped_str(text)))?
                }
                Node::DocType(text) => {
                    writer.write_event(Event::DocType(BytesText::from_escaped_str(text)))?
                }
                Node::PI(text) => {
                    writer.write_event(Event::PI(BytesText::from_escaped_str(text)))?
                }
                Node::Decl {
                    version,
                    encoding,
                    standalone,
                } => writer.write_event(Event::Decl(BytesDecl::new(
                    version.as_bytes(),
                    encoding.as_ref().map(|s| s.as_bytes()),
                    standalone.as_ref().map(|s| s.as_bytes()),
                )))?,
            };
        }
        Ok(())
    }

    fn write_element(&self, writer: &mut Writer<impl Write>, element: Element) -> Result<()> {
        let name_bytes = element.raw_name(self).as_bytes();
        let mut start = BytesStart::borrowed_name(name_bytes);
        for (key, val) in element.attributes(self) {
            start.push_attribute((key.as_bytes(), val.as_bytes()));
        }
        for (prefix, val) in element.namespace_declarations(self) {
            let attr_name = if prefix.is_empty() {
                "xmlns".to_string()
            } else {
                format!("xmlns:{}", prefix)
            };
            start.push_attribute((attr_name.as_bytes(), val.as_bytes()));
        }
        if element.has_children(self) {
            writer.write_event(Event::Start(start))?;
            self.write_nodes(writer, element.children(self))?;
            writer.write_event(Event::End(BytesEnd::borrowed(name_bytes)))?;
        } else {
            writer.write_event(Event::Empty(start))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_element() {
        let xml = r#"
        <basic>
            Text
            <c />
        </basic>
        "#;
        let mut document = Document::from_str(xml).unwrap();
        let basic = document.nodes[0].as_element().unwrap();
        let p = Element::new(&mut document, "p");
        basic.push_child(&mut document, Node::Element(p)).unwrap();
        assert_eq!(p.parent(&document).unwrap(), basic);
        assert_eq!(
            p,
            basic
                .children(&document)
                .last()
                .unwrap()
                .as_element()
                .unwrap()
        )
    }

    #[test]
    fn test_namespace() {
        let xml = r#"
        <root xmlns="ns", xmlns:p="pns">
            <p:foo xmlns="inner">
                Hello
            </p:foo>
            <p:bar xmlns:p="in2">
                <c />
                World!
            </p:bar>
        </root>"#;
        let doc = Document::from_str(xml).unwrap();
        let root = doc.nodes[0].as_element().unwrap();
        let child_elements = root.child_elements(&doc);
        let foo = *child_elements.get(0).unwrap();
        let bar = *child_elements.get(1).unwrap();
        let c = bar.child_elements(&doc)[0];
        assert_eq!(c.prefix_name(&doc), ("", "c"));
        assert_eq!(bar.raw_name(&doc), "p:bar");
        assert_eq!(bar.prefix(&doc), "p");
        assert_eq!(bar.name(&doc), "bar");
        assert_eq!(c.namespace(&doc).unwrap(), "ns");
        assert_eq!(c.namespace_for_prefix(&doc, "p").unwrap(), "in2");
        assert!(c.namespace_for_prefix(&doc, "random").is_none());
        assert_eq!(bar.namespace(&doc).unwrap(), "in2");
        assert_eq!(bar.namespace_for_prefix(&doc, "").unwrap(), "ns");
        assert_eq!(foo.namespace(&doc).unwrap(), "pns");
        assert_eq!(foo.namespace_for_prefix(&doc, "").unwrap(), "inner");
        assert_eq!(foo.namespace_for_prefix(&doc, "p").unwrap(), "pns");
        assert_eq!(root.namespace(&doc).unwrap(), "ns");
    }
}
