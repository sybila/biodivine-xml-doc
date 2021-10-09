# xml-doc

xml-doc is a rust library to read, modify, and write XML documents.

It's aim is to be able to read **any** xml files, and modify only the parts you want to.

Features:

- Supports reading from most encodings, including UTF-16. (With the notable exception of UTF-32)
- You can have references to the parts of the tree, and still mutate the tree.
- Elements stores reference to its parent element, so traveling up the tree is fast.
- One of the fastest XML tree-like parser & writer. See [#Performance](https://github.com/bluegreenmagick/xml-doc#performance).
- Supports attribute value normalization, character/entity references.

Due to its architecture, you can't exchange nodes or elements between documents.
If your project modifies multiple xml documents at the same time, this library may not be a good fit.

## Example

```rust
use xml_doc::{Document, Element};

let XML = r#"<?xml version="1.0"?>
<package xmlns:dc="http://purl.org/dc/elements/1.1/">
    <metadata>
        <dc:title>xml-doc</dc:title>
        <dc:rights>MIT or Apache 2.0</dc:rights>
    </metadata>
</package>
"#;

let doc = Document::parse_str(XML).unwrap();
let package = doc.root_element().unwrap();
let metadata = package.find(&doc, "metadata").unwrap();
let title = metadata.find(&doc, "title").unwrap();
title.set_attribute("xml:lang", "en");

// Add an element to metadata: <dc:creator id="author">Yoonchae Lee</dc:creator>
let author = Element::build(&mut doc, "dc:creator")
    .text_content("Yoonchae Lee")
    .attribute("id", "author")
    .push_to(metadata);

let new_xml = doc.write_str();
```

## Performance

To run benchmark: `cd benches ; cargo bench`.

### Parsing

```
                  tiny(4.8KB) medium(1.5MB) large(25MB) medium(UTF-16)
xml_doc v0.1.1:     88.40us     32.06ms      370.38ms        33.40ms
minidom v0.12.0:   104.70us     48.80ms      682.06ms
roxmltree v0.14.1:  53.00us     17.40ms      359.34ms
xmltree v0.10.3:  4341.9 us   1352.2 ms    22863. ms
```

Only roxmltree which doesn't support writing, is considerably faster than xml_doc. You can see the result of the benchmarks [here](https://github.com/BlueGreenMagick/xml-doc/runs/3845335396).
