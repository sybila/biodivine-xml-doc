use crate::element::{Element, ElementData};
use crate::error::{Error, Result};
use crate::parser::{DocumentParser, ReadOptions};
use quick_xml::events::{BytesCData, BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Write};
use std::iter::FromIterator;
use std::path::Path;
use std::str::FromStr;

/// Represents an XML node.
#[derive(Debug)]
pub enum Node {
    /// XML Element
    Element(Element),
    /// XML Character Data ([specification](https://www.w3.org/TR/xml/#syntax))
    Text(String),
    /// Comments ([specification](https://www.w3.org/TR/xml/#sec-comments))
    Comment(String),
    /// CDATA ([specification](https://www.w3.org/TR/xml/#sec-cdata-sect))
    CData(String),
    /// Processing Instruction ([specification](https://www.w3.org/TR/xml/#sec-pi))
    PI(String),
    /// Document Type Declaration ([specification](https://www.w3.org/TR/xml/#sec-prolog-dtd))
    DocType(String),
}

impl Node {
    /// Useful to use inside `filter_map`.
    ///
    /// ```rust
    /// use biodivine_xml_doc::{Document, Element};
    ///
    /// let mut doc = Document::parse_str(r#"<?xml version="1.0" encoding="UTF-8"?>
    /// <config>
    ///     Random Text
    ///     <max>1</max>
    /// </config>
    /// "#).unwrap();
    ///
    /// let elems: Vec<Element> = doc
    ///     .root_element()
    ///     .unwrap()
    ///     .children(&doc)
    ///     .iter()
    ///     .filter_map(|n| n.as_element())
    ///     .collect();
    /// ```
    pub fn as_element(&self) -> Option<Element> {
        match self {
            Self::Element(elem) => Some(*elem),
            _ => None,
        }
    }

    pub(crate) fn build_text_content<'a>(&self, doc: &'a Document, buf: &'a mut String) {
        match self {
            Node::Element(elem) => elem.build_text_content(doc, buf),
            Node::Text(text) => buf.push_str(text),
            Node::CData(text) => buf.push_str(text),
            Node::PI(text) => buf.push_str(text),
            _ => {}
        }
    }

    /// Returns content if node is `Text`, `CData`, or `PI`.
    /// If node is `Element`, return [Element::text_content()]
    ///
    /// Implementation of [Node.textContent](https://developer.mozilla.org/en-US/docs/Web/API/Node/textContent)
    pub fn text_content(&self, doc: &Document) -> String {
        let mut buf = String::new();
        self.build_text_content(doc, &mut buf);
        buf
    }
}

/// Represents a XML document or a document fragment.
///
/// To build a document from scratch, use [`Document::new`].
///
/// To read and modify an existing document, use [parse_*](`Document#parsing`) methods.
///
/// To write the document, use [write_*](`Document#writing`) methods.
///
/// # Examples
/// ```rust
/// use biodivine_xml_doc::Document;
///
/// let mut doc = Document::parse_str(r#"<?xml version="1.0" encoding="UTF-8"?>
/// <package>
///     <metadata>
///         <author>Lewis Carol</author>
///     </metadata>
/// </package>
/// "#).unwrap();
/// let author_elem = doc
///   .root_element()
///   .unwrap()
///   .find(&doc, "metadata")
///   .unwrap()
///   .find(&doc, "author")
///   .unwrap();
/// author_elem.set_text_content(&mut doc, "Lewis Carroll");
/// let xml = doc.write_str();
/// ```
///

#[derive(Debug)]
pub struct Document {
    pub(crate) counter: usize, // == self.store.len()
    pub(crate) store: Vec<ElementData>,
    container: Element,

    pub(crate) version: String,
    pub(crate) standalone: bool,
}

impl Default for Document {
    fn default() -> Self {
        Document::new()
    }
}

impl Document {
    /// Create a blank new xml document.
    pub fn new() -> Document {
        let (container, container_data) = Element::container();
        Document {
            counter: 1, // because container is id 0
            store: vec![container_data],
            container,
            version: String::from("1.0"),
            standalone: false,
        }
    }

    /// Get 'container' element of Document.
    ///
    /// The document uses an invisible 'container' element
    /// which it uses to manage its root nodes.
    ///
    /// Its parent is None, and trying to change its parent will
    /// return [`Error::ContainerCannotMove`].
    ///
    /// For the container element, only its `children` is relevant.
    /// Other attributes are not used.
    pub fn container(&self) -> Element {
        self.container
    }

    /// Returns `true` if document doesn't have any nodes.
    /// Returns `false` if you added a node or parsed an xml.
    ///
    /// You can only call `parse_*()` if document is empty.
    pub fn is_empty(&self) -> bool {
        self.store.len() == 1
    }

    /// Get root nodes of document.
    pub fn root_nodes(&self) -> &Vec<Node> {
        self.container.children(self)
    }

    /// Get first root node that is an element.
    pub fn root_element(&self) -> Option<Element> {
        self.container.child_elements(self).first().copied()
    }

    /// Push a node to end of root nodes.
    /// If doc has no [`Element`], pushing a [`Node::Element`] is
    /// equivalent to setting it as root element.
    pub fn push_root_node(&mut self, node: Node) -> Result<()> {
        let elem = self.container;
        elem.push_child(self, node)
    }
}

/// &nbsp;
/// # Parsing
///
/// Below are methods for parsing xml.
/// Parsing from string, file, and reader is supported.
///
/// Call `parse_*_with_opts` with custom [`ReadOptions`] to change parser behaviour.
/// Otherwise, [`ReadOptions::default()`] is used.
///
/// # Errors
/// - [`Error::CannotDecode`]: Could not decode XML. XML declaration may have invalid encoding value.
/// - [`Error::MalformedXML`]: Could not read XML.
/// - [`Error::Io`]: IO Error
impl Document {
    pub fn parse_str(str: &str) -> Result<Document> {
        DocumentParser::parse_reader(str.as_bytes(), ReadOptions::default())
    }
    pub fn parse_str_with_opts(str: &str, opts: ReadOptions) -> Result<Document> {
        DocumentParser::parse_reader(str.as_bytes(), opts)
    }

    pub fn parse_file<P: AsRef<Path>>(path: P) -> Result<Document> {
        let file = File::open(path)?;
        DocumentParser::parse_reader(file, ReadOptions::default())
    }
    pub fn parse_file_with_opts<P: AsRef<Path>>(path: P, opts: ReadOptions) -> Result<Document> {
        let file = File::open(path)?;
        DocumentParser::parse_reader(file, opts)
    }

    pub fn parse_reader<R: Read>(reader: R) -> Result<Document> {
        DocumentParser::parse_reader(reader, ReadOptions::default())
    }
    pub fn parse_reader_with_opts<R: Read>(reader: R, opts: ReadOptions) -> Result<Document> {
        DocumentParser::parse_reader(reader, opts)
    }
}

/// Options when writing XML.
pub struct WriteOptions {
    /// Byte character to indent with. (default: `b' '`)
    pub indent_char: u8,
    /// How many indent_char should be used for indent. (default: 2)
    pub indent_size: usize,
    /// XML declaration should be written at the top. (default: `true`)
    pub write_decl: bool,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions {
            indent_char: b' ',
            indent_size: 2,
            write_decl: true,
        }
    }
}

/// &nbsp;
/// # Writing
///
/// Below are methods for writing xml.
/// The XML will be written in UTF-8.
impl Document {
    pub fn write_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        self.write_file_with_opts(path, WriteOptions::default())
    }
    pub fn write_file_with_opts<P: AsRef<Path>>(&self, path: P, opts: WriteOptions) -> Result<()> {
        let mut file = File::create(path)?;
        self.write_with_opts(&mut file, opts)
    }

    pub fn write_str(&self) -> Result<String> {
        self.write_str_with_opts(WriteOptions::default())
    }
    pub fn write_str_with_opts(&self, opts: WriteOptions) -> Result<String> {
        let mut buf: Vec<u8> = Vec::with_capacity(200);
        self.write_with_opts(&mut buf, opts)?;
        Ok(String::from_utf8(buf)?)
    }

    pub fn write(&self, writer: &mut impl Write) -> Result<()> {
        self.write_with_opts(writer, WriteOptions::default())
    }
    pub fn write_with_opts(&self, writer: &mut impl Write, opts: WriteOptions) -> Result<()> {
        let container = self.container();
        let mut writer = Writer::new_with_indent(writer, opts.indent_char, opts.indent_size);
        if opts.write_decl {
            self.write_decl(&mut writer)?;
        }
        self.write_nodes(&mut writer, container.children(self))?;
        writer.write_event(Event::Eof)?;
        Ok(())
    }

    fn write_decl(&self, writer: &mut Writer<impl Write>) -> Result<()> {
        let standalone = match self.standalone {
            true => Some("yes"),
            false => None,
        };
        writer.write_event(Event::Decl(BytesDecl::new(
            self.version.as_str(),
            Some("UTF-8"),
            standalone,
        )))?;
        Ok(())
    }

    fn write_nodes(&self, writer: &mut Writer<impl Write>, nodes: &[Node]) -> Result<()> {
        for node in nodes {
            match node {
                Node::Element(eid) => self.write_element(writer, *eid)?,
                Node::Text(text) => writer.write_event(Event::Text(BytesText::new(text)))?,
                Node::DocType(text) => writer.write_event(Event::DocType(BytesText::new(text)))?,
                // Comment, CData, and PI content is not escaped.
                Node::Comment(text) => {
                    writer.write_event(Event::Comment(BytesText::from_escaped(text)))?
                }
                Node::CData(text) => writer.write_event(Event::CData(BytesCData::new(text)))?,
                Node::PI(text) => writer.write_event(Event::PI(BytesText::from_escaped(text)))?,
            };
        }
        Ok(())
    }

    fn write_element(&self, writer: &mut Writer<impl Write>, element: Element) -> Result<()> {
        let name_str = element.full_name(self);
        let mut start = BytesStart::new(name_str);
        // The copy in BTreeMap ensures that we have a deterministic iteration order.
        let attributes = BTreeMap::from_iter(element.attributes(self).iter());
        for (key, val) in attributes {
            start.push_attribute((key.as_str(), val.as_str()));
        }
        let namespaces = BTreeMap::from_iter(element.namespace_decls(self).iter());
        for (prefix, val) in namespaces {
            let attr_name = if prefix.is_empty() {
                "xmlns".to_string()
            } else {
                format!("xmlns:{}", prefix)
            };
            start.push_attribute((attr_name.as_str(), val.as_str()));
        }
        if element.has_children(self) {
            writer.write_event(Event::Start(start))?;
            self.write_nodes(writer, element.children(self))?;
            writer.write_event(Event::End(BytesEnd::new(name_str)))?;
        } else {
            writer.write_event(Event::Empty(start))?;
        }
        Ok(())
    }
}

impl FromStr for Document {
    type Err = Error;

    fn from_str(s: &str) -> Result<Document> {
        Document::parse_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_element() {
        let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
        <basic>
            Text
            <c />
        </basic>
        "#;
        let mut doc = Document::from_str(xml).unwrap();
        let basic = doc.container().children(&doc)[0].as_element().unwrap();
        let p = Element::new(&mut doc, "p");
        basic.push_child(&mut doc, Node::Element(p)).unwrap();
        assert_eq!(p.parent(&doc).unwrap(), basic);
        assert_eq!(
            p,
            basic.children(&doc).last().unwrap().as_element().unwrap()
        )
    }

    #[test]
    fn test_enforce_encoding() {
        // This document can be parsed without issues if we don't require a specific encoding,
        // but it is not UTF-8 and hence should fail if we specifically request UTF-8.
        let xml = "<?xml version=\"1.0\" encoding=\"US-ASCII\"?><test></test>";
        assert!(Document::parse_str(xml).is_ok());
        let mut opts = ReadOptions::default();
        opts.enforce_encoding = true;
        // We have not specified any encoding, hence this should always fail.
        assert!(matches!(
            Document::parse_str_with_opts(xml, opts.clone()),
            Err(Error::CannotDecode)
        ));
        // With the correct encoding, this should now work.
        opts.encoding = Some("US-ASCII".to_string());
        let doc = Document::parse_str_with_opts(xml, opts.clone()).unwrap();
        assert_eq!(doc.root_element().unwrap().name(&doc), "test");
        // But with a different encoding, we should fail again.
        opts.encoding = Some("UTF-8".to_string());
        assert!(matches!(
            Document::parse_str_with_opts(xml, opts.clone()),
            Err(Error::CannotDecode)
        ));

        // Do a similar thing with a UTF document, because UTF gets special treatment in the
        // library logic.
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-8\"?><test></test>";
        assert!(Document::parse_str(xml).is_ok());
        let mut opts = ReadOptions::default();
        opts.enforce_encoding = true;
        assert!(matches!(
            Document::parse_str_with_opts(xml, opts.clone()),
            Err(Error::CannotDecode)
        ));
        opts.encoding = Some("US-ASCII".to_string());
        assert!(matches!(
            Document::parse_str_with_opts(xml, opts.clone()),
            Err(Error::CannotDecode)
        ));
        opts.encoding = Some("UTF-8".to_string());
        let doc = Document::parse_str_with_opts(xml, opts.clone()).unwrap();
        assert_eq!(doc.root_element().unwrap().name(&doc), "test");
    }
}
